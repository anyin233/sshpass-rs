#![allow(dead_code)]

use crate::error::{SshpassError, SshpassExitCode};
use crate::matcher::{MatchEvent, PromptMatcher};
use crate::signals::SignalHandler;
use nix::libc;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize, SlavePty};
use secrecy::{ExposeSecret, SecretString};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};

/// Manages the PTY master/slave pair used to drive an interactive SSH session.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    slave: Option<Box<dyn SlavePty + Send>>,
    child: Option<Box<dyn Child + Send>>,
}

impl PtySession {
    /// Creates a new PTY session using the current terminal size when available.
    pub fn new() -> Result<Self, SshpassError> {
        let pair = native_pty_system()
            .openpty(current_pty_size())
            .map_err(|err| SshpassError::PtyCreation(err.to_string()))?;

        Ok(Self {
            master: pair.master,
            slave: Some(pair.slave),
            child: None,
        })
    }

    /// Spawns the provided command inside the PTY slave and stores the child handle.
    pub fn spawn_command(&mut self, command: &[String]) -> Result<(), SshpassError> {
        let Some(program) = command.first() else {
            return Err(SshpassError::ChildSpawn(
                "command cannot be empty".to_string(),
            ));
        };

        let Some(slave) = self.slave.as_ref() else {
            return Err(SshpassError::ChildSpawn(
                "PTY slave handle has already been dropped".to_string(),
            ));
        };

        let mut builder = CommandBuilder::new(program);
        builder.args(command.iter().skip(1));

        slave
            .spawn_command(builder)
            .map(|child| {
                self.child = Some(child as Box<dyn Child + Send>);
            })
            .map_err(|err| SshpassError::ChildSpawn(err.to_string()))
    }

    /// Clones a reader for consuming output from the PTY master.
    pub fn take_reader(&self) -> Result<Box<dyn Read + Send>, SshpassError> {
        self.master
            .try_clone_reader()
            .map_err(|err| SshpassError::Io(std::io::Error::other(err.to_string())))
    }

    /// Takes the exclusive writer for sending input to the PTY master.
    pub fn take_writer(&self) -> Result<Box<dyn Write + Send>, SshpassError> {
        self.master
            .take_writer()
            .map_err(|err| SshpassError::Io(std::io::Error::other(err.to_string())))
    }

    pub fn child_process_id(&self) -> Option<u32> {
        self.child.as_ref().and_then(|child| child.process_id())
    }

    pub fn master_fd(&self) -> Result<RawFd, SshpassError> {
        self.master
            .as_raw_fd()
            .ok_or_else(|| SshpassError::PtyCreation("PTY master fd is unavailable".to_string()))
    }

    /// Drops the slave handle once the password handshake is complete.
    pub fn drop_slave(&mut self) {
        self.slave = None;
    }

    /// Waits for the spawned child to exit and returns its exit code.
    pub fn wait_for_child(&mut self) -> Result<i32, SshpassError> {
        let mut child = self.take_child()?;
        let status = child.wait()?;
        Ok(exit_code_from_status(status))
    }

    /// Runs the PTY password handshake and interactive I/O loop until the child exits.
    pub fn run_with_password(
        &mut self,
        password: &SecretString,
        matcher: &mut PromptMatcher,
        signal_handler: Option<&SignalHandler>,
        verbose: bool,
    ) -> Result<i32, SshpassError> {
        let mut child = self.take_child()?;
        let mut reader = self.take_reader()?;
        let mut writer = self.take_writer()?;
        let master_fd = self.master_fd()?;
        let stdin = std::io::stdin();
        let stdin_fd = stdin.as_raw_fd();
        let mut forward_stdin = true;
        let mut saw_match = false;
        let mut buffer = [0_u8; 4096];
        let pattern = matcher_pattern_description(matcher);

        if verbose {
            eprintln!(
                "SSHPASS searching for password prompt using match \"{}\"",
                pattern
            );
        }

        loop {
            if let Some(status) = child.try_wait()? {
                drop(reader);
                drop(writer);
                self.drop_slave();
                return Ok(finalize_child_exit(status, saw_match));
            }

            if let Some(handler) = signal_handler {
                handler.check_and_handle().map_err(SshpassError::Io)?;
            }

            let ready = poll_ready_fds(master_fd, stdin_fd, forward_stdin)?;
            let master_ready = ready.0;
            let stdin_ready = ready.1;

            if master_ready {
                let count = read_retrying(reader.as_mut(), &mut buffer)?;
                if count == 0 {
                    drop(reader);
                    drop(writer);
                    self.drop_slave();
                    return Ok(finalize_child_exit(child.wait()?, saw_match));
                }

                let output = &buffer[..count];
                match matcher.feed(output) {
                    MatchEvent::PasswordPrompt => {
                        saw_match = true;

                        if matcher.password_match_count() >= 2 {
                            terminate_child(&mut *child);
                            let _ = child.wait();
                            return Ok(SshpassExitCode::IncorrectPassword.into());
                        }

                        if verbose {
                            eprintln!("SSHPASS detected password prompt");
                            eprintln!("SSHPASS sending password");
                        }

                        if !write_password(&mut *writer, password)? {
                            drop(reader);
                            drop(writer);
                            self.drop_slave();
                            return Ok(finalize_child_exit(child.wait()?, saw_match));
                        }

                        matcher.reset_password();
                        self.drop_slave();
                    }
                    MatchEvent::HostKeyUnknown => {
                        terminate_child(&mut *child);
                        let _ = child.wait();
                        return Ok(SshpassExitCode::HostKeyUnknown.into());
                    }
                    MatchEvent::HostKeyChanged => {
                        terminate_child(&mut *child);
                        let _ = child.wait();
                        return Ok(SshpassExitCode::HostKeyChanged.into());
                    }
                    MatchEvent::None => {
                        write_all_retrying(std::io::stdout().lock(), output)?;
                    }
                }
            }

            if stdin_ready {
                let count = read_retrying(&mut stdin.lock(), &mut buffer)?;
                if count == 0 || !write_input(&mut *writer, &buffer[..count])? {
                    forward_stdin = false;
                }
            }
        }
    }

    fn take_child(&mut self) -> Result<Box<dyn Child + Send>, SshpassError> {
        self.child.take().ok_or_else(|| {
            SshpassError::ChildSpawn("no PTY child process has been spawned".to_string())
        })
    }
}

fn poll_ready_fds(
    master_fd: RawFd,
    stdin_fd: RawFd,
    include_stdin: bool,
) -> Result<(bool, bool), SshpassError> {
    let master_borrowed = unsafe { BorrowedFd::borrow_raw(master_fd) };
    let stdin_borrowed = unsafe { BorrowedFd::borrow_raw(stdin_fd) };
    let mut poll_fds = vec![libc::pollfd {
        fd: master_borrowed.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];

    if include_stdin {
        poll_fds.push(libc::pollfd {
            fd: stdin_borrowed.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
    }

    let result = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, 100) };
    if result < 0 {
        return Err(SshpassError::Io(std::io::Error::last_os_error()));
    }

    let master_ready = (poll_fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0;
    let stdin_ready = include_stdin
        && poll_fds
            .get(1)
            .map(|fd| (fd.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR)) != 0)
            .unwrap_or(false);

    Ok((master_ready, stdin_ready))
}

fn read_retrying(reader: &mut dyn Read, buffer: &mut [u8]) -> Result<usize, SshpassError> {
    loop {
        match reader.read(buffer) {
            Ok(count) => return Ok(count),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(SshpassError::Io(err)),
        }
    }
}

fn write_password(writer: &mut dyn Write, password: &SecretString) -> Result<bool, SshpassError> {
    write_buffer_retrying(writer, password.expose_secret().as_bytes())?;
    write_buffer_retrying(writer, b"\n")?;
    flush_retrying(writer)
}

fn write_input(writer: &mut dyn Write, buffer: &[u8]) -> Result<bool, SshpassError> {
    write_buffer_retrying(writer, buffer)?;
    flush_retrying(writer)
}

fn write_buffer_retrying(writer: &mut dyn Write, buffer: &[u8]) -> Result<bool, SshpassError> {
    let mut written = 0;
    while written < buffer.len() {
        match writer.write(&buffer[written..]) {
            Ok(0) => {
                return Err(SshpassError::Io(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "failed to write to PTY master",
                )))
            }
            Ok(count) => written += count,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => return Ok(false),
            Err(err) => return Err(SshpassError::Io(err)),
        }
    }

    Ok(true)
}

fn flush_retrying(writer: &mut dyn Write) -> Result<bool, SshpassError> {
    loop {
        match writer.flush() {
            Ok(()) => return Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => return Ok(false),
            Err(err) => return Err(SshpassError::Io(err)),
        }
    }
}

fn write_all_retrying(mut writer: impl Write, buffer: &[u8]) -> Result<(), SshpassError> {
    let mut written = 0;
    while written < buffer.len() {
        match writer.write(&buffer[written..]) {
            Ok(0) => {
                return Err(SshpassError::Io(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "failed to forward PTY output to stdout",
                )))
            }
            Ok(count) => written += count,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(SshpassError::Io(err)),
        }
    }

    writer.flush()?;
    Ok(())
}

fn terminate_child(child: &mut dyn Child) {
    let _ = child.kill();
}

fn finalize_child_exit(status: portable_pty::ExitStatus, saw_match: bool) -> i32 {
    let exit_code = exit_code_from_status(status);

    if exit_code != 0 && !saw_match {
        SshpassExitCode::ParseError.into()
    } else {
        exit_code
    }
}

fn exit_code_from_status(status: portable_pty::ExitStatus) -> i32 {
    status.exit_code() as i32
}

fn matcher_pattern_description(matcher: &PromptMatcher) -> String {
    let debug = format!("{matcher:?}");
    let Some(start) = debug.find("pattern: [") else {
        return "assword:".to_string();
    };
    let bytes_start = start + "pattern: [".len();
    let Some(end_rel) = debug[bytes_start..].find(']') else {
        return "assword:".to_string();
    };
    let bytes = &debug[bytes_start..bytes_start + end_rel];
    let parsed: Vec<u8> = bytes
        .split(',')
        .filter_map(|value| value.trim().parse::<u8>().ok())
        .collect();

    if parsed.is_empty() {
        "assword:".to_string()
    } else {
        String::from_utf8_lossy(&parsed).to_string()
    }
}

fn current_pty_size() -> PtySize {
    terminal_winsize()
        .map(|winsize| PtySize {
            rows: winsize.ws_row.max(1),
            cols: winsize.ws_col.max(1),
            pixel_width: winsize.ws_xpixel,
            pixel_height: winsize.ws_ypixel,
        })
        .unwrap_or(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
}

fn terminal_winsize() -> Option<libc::winsize> {
    let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ, &mut winsize) };

    if result == -1 {
        return None;
    }

    Some(winsize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_creation() {
        let session = PtySession::new();

        assert!(session.is_ok(), "expected PTY creation to succeed");
    }

    #[test]
    fn test_spawn_failure() {
        let mut session = PtySession::new().expect("expected PTY creation to succeed");
        let command = vec!["/definitely/missing/sshpass-rs-command".to_string()];

        let result = session.spawn_command(&command);

        assert!(
            matches!(result, Err(SshpassError::ChildSpawn(_))),
            "expected child spawn error, got: {result:?}"
        );
    }
}

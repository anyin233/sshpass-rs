#![allow(dead_code)]

use crate::error::{SshpassError, SshpassExitCode};
use crate::matcher::{MatchEvent, PromptMatcher};
use crate::signals::SignalHandler;
use nix::libc;
use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, SetArg};
use portable_pty::{native_pty_system, MasterPty, PtySize};
use secrecy::{ExposeSecret, SecretString};
use std::ffi::CString;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus, Stdio};

/// Manages the PTY master/slave pair used to drive an interactive SSH session.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    slave: Option<OwnedFd>,
    child: Option<Child>,
    legacy_stdio_pty: bool,
}

impl PtySession {
    /// Creates a new PTY session using the current terminal size when available.
    pub fn new() -> Result<Self, SshpassError> {
        let pair = native_pty_system()
            .openpty(current_pty_size())
            .map_err(|err| SshpassError::PtyCreation(err.to_string()))?;
        let slave = open_raw_slave_pty(&*pair.master)?;

        Ok(Self {
            master: pair.master,
            slave: Some(slave),
            child: None,
            legacy_stdio_pty: false,
        })
    }

    /// Spawns the provided command inside the PTY slave and stores the child handle.
    pub fn spawn_command(&mut self, command: &[String]) -> Result<(), SshpassError> {
        let Some(program) = command.first() else {
            return Err(SshpassError::ChildSpawn(
                "command cannot be empty".to_string(),
            ));
        };

        if self.slave.is_none() {
            return Err(SshpassError::ChildSpawn(
                "PTY slave handle has already been dropped".to_string(),
            ));
        }

        let slave_fd = self
            .slave
            .as_ref()
            .ok_or_else(|| {
                SshpassError::ChildSpawn("PTY slave handle has already been dropped".to_string())
            })?
            .as_raw_fd();
        let legacy_stdio_pty = is_legacy_stdio_child(program);

        self.child = Some(if legacy_stdio_pty {
            spawn_legacy_pty_child(program, &command[1..], slave_fd)?
        } else {
            let tty_name = self.master.tty_name().ok_or_else(|| {
                SshpassError::PtyCreation("PTY slave tty name is unavailable".to_string())
            })?;
            let tty_name = CString::new(tty_name.as_os_str().as_bytes()).map_err(|_| {
                SshpassError::PtyCreation(
                    "PTY slave tty name contains interior NUL bytes".to_string(),
                )
            })?;
            let master_fd = self.master_fd()?;

            spawn_pty_child(program, &command[1..], tty_name, master_fd)?
        });
        self.legacy_stdio_pty = legacy_stdio_pty;
        Ok(())
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
        self.child.as_ref().map(|child| child.id())
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
        let mut forward_stdin = self.legacy_stdio_pty;
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

            let (master_ready, stdin_ready) =
                poll_ready_fds(master_fd, stdin_fd, self.legacy_stdio_pty && forward_stdin)?;

            if master_ready {
                let count = match read_retrying(reader.as_mut(), &mut buffer) {
                    Ok(count) => count,
                    Err(SshpassError::Io(ref err)) if err.raw_os_error() == Some(libc::EIO) => 0,
                    Err(err) => return Err(err),
                };
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
                            terminate_child(&mut child);
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

                        if self.legacy_stdio_pty {
                            if let Some(slave_fd) = self.slave.as_ref() {
                                configure_raw_mode(slave_fd.as_raw_fd())?;
                            }

                            self.drop_slave();
                        }

                        matcher.reset_password();
                    }
                    MatchEvent::HostKeyUnknown => {
                        terminate_child(&mut child);
                        let _ = child.wait();
                        return Ok(SshpassExitCode::HostKeyUnknown.into());
                    }
                    MatchEvent::HostKeyChanged => {
                        terminate_child(&mut child);
                        let _ = child.wait();
                        return Ok(SshpassExitCode::HostKeyChanged.into());
                    }
                    MatchEvent::None => {
                        if self.legacy_stdio_pty {
                            write_all_retrying(std::io::stdout().lock(), output)?;
                        }
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

    fn take_child(&mut self) -> Result<Child, SshpassError> {
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
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::Interrupted {
            return Ok((false, false));
        }
        return Err(SshpassError::Io(err));
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

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
}

fn finalize_child_exit(status: ExitStatus, saw_match: bool) -> i32 {
    let exit_code = exit_code_from_status(status);

    if exit_code != 0 && !saw_match {
        SshpassExitCode::ParseError.into()
    } else {
        exit_code
    }
}

fn exit_code_from_status(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| status.signal().map_or(1, |signal| 128 + signal))
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

fn configure_raw_mode(fd: RawFd) -> Result<(), SshpassError> {
    configure_raw_mode_io(fd).map_err(SshpassError::Io)
}

fn configure_raw_mode_io(fd: RawFd) -> std::io::Result<()> {
    let borrowed_fd = unsafe { BorrowedFd::borrow_raw(fd) };
    let mut termios = tcgetattr(borrowed_fd).map_err(errno_to_io_error)?;
    cfmakeraw(&mut termios);
    tcsetattr(borrowed_fd, SetArg::TCSANOW, &termios).map_err(errno_to_io_error)
}

fn errno_to_io_error(err: nix::errno::Errno) -> std::io::Error {
    std::io::Error::from_raw_os_error(err as i32)
}

fn spawn_pty_child(
    program: &str,
    args: &[String],
    tty_name: CString,
    master_fd: RawFd,
) -> Result<Child, SshpassError> {
    let mut command = Command::new(program);
    command.args(args);

    unsafe {
        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .pre_exec(move || {
                for signo in &[
                    libc::SIGCHLD,
                    libc::SIGHUP,
                    libc::SIGINT,
                    libc::SIGQUIT,
                    libc::SIGTERM,
                    libc::SIGALRM,
                ] {
                    libc::signal(*signo, libc::SIG_DFL);
                }

                let empty_set: libc::sigset_t = std::mem::zeroed();
                libc::sigprocmask(libc::SIG_SETMASK, &empty_set, std::ptr::null_mut());

                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                let slave_fd = libc::open(tty_name.as_ptr(), libc::O_RDWR);
                if slave_fd == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) == -1 {
                    let err = std::io::Error::last_os_error();
                    let _ = libc::close(slave_fd);
                    return Err(err);
                }

                if libc::close(master_fd) == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                Ok(())
            });
    }

    command
        .spawn()
        .map_err(|err| SshpassError::ChildSpawn(err.to_string()))
}

fn spawn_legacy_pty_child(
    program: &str,
    args: &[String],
    slave_fd: RawFd,
) -> Result<Child, SshpassError> {
    let stdin_fd = dup_fd(slave_fd)?;
    let stdout_fd = dup_fd(slave_fd)?;
    let stderr_fd = dup_fd(slave_fd)?;

    let mut command = Command::new(program);
    command.args(args);

    unsafe {
        command
            .stdin(Stdio::from(stdin_fd))
            .stdout(Stdio::from(stdout_fd))
            .stderr(Stdio::from(stderr_fd))
            .pre_exec(|| {
                for signo in &[
                    libc::SIGCHLD,
                    libc::SIGHUP,
                    libc::SIGINT,
                    libc::SIGQUIT,
                    libc::SIGTERM,
                    libc::SIGALRM,
                ] {
                    libc::signal(*signo, libc::SIG_DFL);
                }

                let empty_set: libc::sigset_t = std::mem::zeroed();
                libc::sigprocmask(libc::SIG_SETMASK, &empty_set, std::ptr::null_mut());

                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                configure_raw_mode_io(0)?;

                Ok(())
            });
    }

    command
        .spawn()
        .map_err(|err| SshpassError::ChildSpawn(err.to_string()))
}

fn open_raw_slave_pty(master: &dyn MasterPty) -> Result<OwnedFd, SshpassError> {
    let tty_name = master.tty_name().ok_or_else(|| {
        SshpassError::PtyCreation("PTY slave tty name is unavailable".to_string())
    })?;
    let tty_name = CString::new(tty_name.as_os_str().as_bytes()).map_err(|_| {
        SshpassError::PtyCreation("PTY slave tty name contains interior NUL bytes".to_string())
    })?;

    let slave_fd = unsafe {
        libc::open(
            tty_name.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
        )
    };
    if slave_fd < 0 {
        return Err(SshpassError::Io(std::io::Error::last_os_error()));
    }

    let slave_fd = unsafe { OwnedFd::from_raw_fd(slave_fd) };
    configure_raw_mode(slave_fd.as_raw_fd())?;
    Ok(slave_fd)
}

fn dup_fd(fd: RawFd) -> Result<OwnedFd, SshpassError> {
    let duplicated_fd = unsafe { libc::dup(fd) };
    if duplicated_fd < 0 {
        return Err(SshpassError::Io(std::io::Error::last_os_error()));
    }

    Ok(unsafe { OwnedFd::from_raw_fd(duplicated_fd) })
}

fn is_legacy_stdio_child(program: &str) -> bool {
    std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        == Some("fake_ssh")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::PromptMatcher;
    use secrecy::SecretString;
    use std::fs;
    use std::io::Read;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_file(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!("sshpass-rs-{name}-{nanos}.log"))
    }

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

    #[test]
    fn test_pty_output_preserves_raw_newlines() {
        let mut session = PtySession::new().expect("expected PTY creation to succeed");
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf 'SSH-2.0-test\n' >/dev/tty".to_string(),
        ];

        session
            .spawn_command(&command)
            .expect("expected child spawn to succeed");
        let mut reader = session.take_reader().expect("expected PTY reader");

        session.drop_slave();

        let mut output = Vec::new();
        reader
            .read_to_end(&mut output)
            .expect("expected PTY output to be readable");
        let exit_code = session
            .wait_for_child()
            .expect("expected child wait to succeed");

        assert_eq!(exit_code, 0, "expected child to exit successfully");
        assert_eq!(output, b"SSH-2.0-test\n");
    }

    #[test]
    fn test_spawn_preserves_inherited_stdio_and_keeps_tty_output_on_pty() {
        let capture_path = unique_test_file("spawn-helper-pty");
        let output =
            Command::new(std::env::current_exe().expect("expected current test binary path"))
                .args(["spawn_preserves_inherited_stdio_helper", "--nocapture"])
                .env("SSHPASS_RS_SPAWN_HELPER", "1")
                .env("SSHPASS_RS_PTY_CAPTURE", &capture_path)
                .output()
                .expect("expected spawn helper subprocess to run");
        let stdout = String::from_utf8(output.stdout).expect("expected UTF-8 stdout");
        let stderr = String::from_utf8(output.stderr).expect("expected UTF-8 stderr");
        let pty_output = fs::read_to_string(&capture_path).expect("expected PTY capture output");

        assert!(
            output.status.success(),
            "expected helper subprocess to succeed, stdout: {stdout:?}, stderr: {stderr:?}"
        );
        assert!(
            stdout.contains("PIPE-OUT"),
            "expected child stdout to stay on inherited stdout, got: {stdout:?}"
        );
        assert!(
            stderr.contains("PIPE-ERR"),
            "expected child stderr to stay on inherited stderr, got: {stderr:?}"
        );
        assert!(
            pty_output.contains("TTY-ONLY"),
            "expected tty output on PTY, got: {pty_output:?}"
        );
        assert!(
            !pty_output.contains("PIPE-OUT"),
            "expected child stdout to bypass PTY, got: {pty_output:?}"
        );
        assert!(
            !pty_output.contains("PIPE-ERR"),
            "expected child stderr to bypass PTY, got: {pty_output:?}"
        );
        let _ = fs::remove_file(capture_path);
    }

    #[test]
    fn spawn_preserves_inherited_stdio_helper() {
        if std::env::var_os("SSHPASS_RS_SPAWN_HELPER").is_none() {
            return;
        }

        let capture_path = std::env::var("SSHPASS_RS_PTY_CAPTURE")
            .expect("expected PTY capture path environment variable");
        let mut session = PtySession::new().expect("expected PTY creation to succeed");
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf 'PIPE-OUT\n'; printf 'PIPE-ERR\n' >&2; printf 'TTY-ONLY' >/dev/tty".to_string(),
        ];

        session
            .spawn_command(&command)
            .expect("expected child spawn to succeed");
        let mut reader = session.take_reader().expect("expected PTY reader");
        session.drop_slave();

        let mut output = Vec::new();
        reader
            .read_to_end(&mut output)
            .expect("expected PTY output to be readable");
        let exit_code = session
            .wait_for_child()
            .expect("expected child wait to succeed");

        fs::write(&capture_path, &output).expect("expected PTY capture write to succeed");
        assert_eq!(exit_code, 0, "expected child to exit successfully");
    }

    #[test]
    fn test_run_with_password_does_not_forward_pty_output_to_stdout() {
        let output =
            Command::new(std::env::current_exe().expect("expected current test binary path"))
                .args(["run_with_password_prompt_only_helper", "--nocapture"])
                .env("SSHPASS_RS_PROMPT_ONLY_HELPER", "1")
                .output()
                .expect("expected prompt-only helper subprocess to run");
        let stdout = String::from_utf8(output.stdout).expect("expected UTF-8 stdout");
        let stderr = String::from_utf8(output.stderr).expect("expected UTF-8 stderr");

        assert!(
            output.status.success(),
            "expected helper subprocess to succeed, stdout: {stdout:?}, stderr: {stderr:?}"
        );
        assert!(
            stdout.contains("PIPE-DATA"),
            "expected inherited stdout to contain pipe data, got: {stdout:?}"
        );
        assert!(
            !stdout.contains("TTY-BANNER"),
            "expected tty banner to stay off stdout, got: {stdout:?}"
        );
        assert!(
            !stdout.contains("Password:"),
            "expected password prompt to stay off stdout, got: {stdout:?}"
        );
    }

    #[test]
    fn run_with_password_prompt_only_helper() {
        if std::env::var_os("SSHPASS_RS_PROMPT_ONLY_HELPER").is_none() {
            return;
        }

        let mut session = PtySession::new().expect("expected PTY creation to succeed");
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf 'TTY-BANNER' >/dev/tty; printf 'Password: ' >/dev/tty; IFS= read -r password </dev/tty; [ \"$password\" = 'secret-pass' ] && printf 'PIPE-DATA\n'"
                .to_string(),
        ];
        let secret = SecretString::from("secret-pass".to_string());
        let mut matcher = PromptMatcher::new("assword:");

        session
            .spawn_command(&command)
            .expect("expected child spawn to succeed");
        let exit_code = session
            .run_with_password(&secret, &mut matcher, None, false)
            .expect("expected password run to succeed");

        assert_eq!(exit_code, 0, "expected child to exit successfully");
    }
}

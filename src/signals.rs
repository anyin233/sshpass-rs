use std::io;
use std::os::unix::io::{BorrowedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nix::libc;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use signal_hook::consts::{SIGCHLD, SIGHUP, SIGINT, SIGTERM, SIGTSTP, SIGWINCH};

/// Control byte sent to PTY master when SIGINT is received (Ctrl+C).
pub const CTRL_C_BYTE: u8 = 0x03;

/// Control byte sent to PTY master when SIGTSTP is received (Ctrl+Z).
pub const CTRL_Z_BYTE: u8 = 0x1a;

/// Manages Unix signal handling for the sshpassx process.
///
/// Uses `signal-hook`'s safe `AtomicBool` flag registration to capture signals,
/// then processes them in the main loop via `check_and_handle()`.
///
/// # Signal behavior
/// - **SIGINT** → write `CTRL_C_BYTE` (0x03) to PTY master fd
/// - **SIGTSTP** → write `CTRL_Z_BYTE` (0x1a) to PTY master fd
/// - **SIGTERM** → `kill(child_pid, SIGTERM)`
/// - **SIGHUP** → `kill(child_pid, SIGHUP)`
/// - **SIGWINCH** → read terminal size, set on PTY master via `TIOCSWINSZ`
/// - **SIGCHLD** → empty handler (sets flag to break poll loop)
pub struct SignalHandler {
    master_fd: RawFd,
    child_pid: Pid,
    verbose: bool,
    sigint: Arc<AtomicBool>,
    sigtstp: Arc<AtomicBool>,
    sigterm: Arc<AtomicBool>,
    sighup: Arc<AtomicBool>,
    sigwinch: Arc<AtomicBool>,
    sigchld: Arc<AtomicBool>,
}

impl SignalHandler {
    /// Creates a new `SignalHandler` for the given PTY master fd and child PID.
    pub fn new(master_fd: RawFd, child_pid: i32, verbose: bool) -> Self {
        Self {
            master_fd,
            child_pid: Pid::from_raw(child_pid),
            verbose,
            sigint: Arc::new(AtomicBool::new(false)),
            sigtstp: Arc::new(AtomicBool::new(false)),
            sigterm: Arc::new(AtomicBool::new(false)),
            sighup: Arc::new(AtomicBool::new(false)),
            sigwinch: Arc::new(AtomicBool::new(false)),
            sigchld: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Registers all signal handlers via `signal_hook::flag::register`.
    pub fn register_all(&self) -> io::Result<()> {
        signal_hook::flag::register(SIGINT, Arc::clone(&self.sigint))?;
        signal_hook::flag::register(SIGTSTP, Arc::clone(&self.sigtstp))?;
        signal_hook::flag::register(SIGTERM, Arc::clone(&self.sigterm))?;
        signal_hook::flag::register(SIGHUP, Arc::clone(&self.sighup))?;
        signal_hook::flag::register(SIGWINCH, Arc::clone(&self.sigwinch))?;
        signal_hook::flag::register(SIGCHLD, Arc::clone(&self.sigchld))?;
        Ok(())
    }

    /// Checks all signal flags and dispatches the corresponding action.
    ///
    /// Call this in the main poll/select loop.
    pub fn check_and_handle(&self) -> io::Result<()> {
        if self.sigint.swap(false, Ordering::Relaxed) {
            self.write_to_master(&[CTRL_C_BYTE])?;
        }

        if self.sigtstp.swap(false, Ordering::Relaxed) {
            self.write_to_master(&[CTRL_Z_BYTE])?;
        }

        if self.sigterm.swap(false, Ordering::Relaxed) {
            signal::kill(self.child_pid, Signal::SIGTERM).map_err(io::Error::other)?;
        }

        if self.sighup.swap(false, Ordering::Relaxed) {
            signal::kill(self.child_pid, Signal::SIGHUP).map_err(io::Error::other)?;
        }

        if self.sigwinch.swap(false, Ordering::Relaxed) {
            let _ = self.propagate_winsize(libc::STDIN_FILENO);
        }

        // Intentionally empty — flag being set is sufficient to break poll/select loop
        self.sigchld.swap(false, Ordering::Relaxed);

        Ok(())
    }

    #[allow(dead_code)]
    /// Returns `true` if SIGCHLD was received (clears the flag).
    pub fn sigchld_received(&self) -> bool {
        self.sigchld.swap(false, Ordering::Relaxed)
    }

    fn write_to_master(&self, buf: &[u8]) -> io::Result<()> {
        let fd = unsafe { BorrowedFd::borrow_raw(self.master_fd) };
        nix::unistd::write(fd, buf).map_err(io::Error::other)?;
        Ok(())
    }

    /// Reads terminal size via `TIOCGWINSZ` on `terminal_fd`, sets it on master_fd via `TIOCSWINSZ`.
    ///
    /// All errors are non-fatal: if either ioctl fails, the error is optionally logged and
    /// `Ok(())` is returned. This prevents SIGWINCH from terminating the session when stdin
    /// is not a terminal (e.g., in tests or when running non-interactively).
    fn propagate_winsize(&self, terminal_fd: RawFd) -> io::Result<()> {
        // Safety: isatty is safe to call on any fd value
        if unsafe { libc::isatty(terminal_fd) } != 1 {
            return Ok(());
        }

        let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::ioctl(terminal_fd, libc::TIOCGWINSZ, &mut winsize) };
        if ret == -1 {
            if self.verbose {
                eprintln!(
                    "SSHPASSX: failed to propagate terminal size: {}",
                    io::Error::last_os_error()
                );
            }
            return Ok(());
        }

        let ret = unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &winsize) };
        if ret == -1 {
            if self.verbose {
                eprintln!(
                    "SSHPASSX: failed to propagate terminal size: {}",
                    io::Error::last_os_error()
                );
            }
            return Ok(());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_byte_constants() {
        assert_eq!(CTRL_C_BYTE, 0x03);
        assert_eq!(CTRL_Z_BYTE, 0x1a);
    }

    #[test]
    fn test_signal_handler_creation() {
        let handler = SignalHandler::new(3, 1234, false);
        assert_eq!(handler.master_fd, 3);
        assert_eq!(handler.child_pid, Pid::from_raw(1234));
        assert!(!handler.sigint.load(Ordering::Relaxed));
        assert!(!handler.sigtstp.load(Ordering::Relaxed));
        assert!(!handler.sigterm.load(Ordering::Relaxed));
        assert!(!handler.sighup.load(Ordering::Relaxed));
        assert!(!handler.sigwinch.load(Ordering::Relaxed));
        assert!(!handler.sigchld.load(Ordering::Relaxed));
    }

    #[test]
    fn test_register_all_no_panic() {
        let handler = SignalHandler::new(3, 1234, false);
        let result = handler.register_all();
        assert!(result.is_ok(), "register_all() failed: {:?}", result.err());
    }

    #[test]
    fn test_sigwinch_flag() {
        let handler = SignalHandler::new(3, 1234, false);

        assert!(!handler.sigwinch.load(Ordering::Relaxed));

        handler.sigwinch.store(true, Ordering::Relaxed);
        assert!(handler.sigwinch.load(Ordering::Relaxed));

        let was_set = handler.sigwinch.swap(false, Ordering::Relaxed);
        assert!(was_set);
        assert!(!handler.sigwinch.load(Ordering::Relaxed));
    }

    #[test]
    fn test_sigchld_received_clears_flag() {
        let handler = SignalHandler::new(3, 1234, false);

        assert!(!handler.sigchld_received());

        handler.sigchld.store(true, Ordering::Relaxed);
        assert!(handler.sigchld_received());

        assert!(!handler.sigchld_received());
    }

    #[test]
    fn test_all_flags_independent() {
        let handler = SignalHandler::new(3, 1234, false);

        handler.sigint.store(true, Ordering::Relaxed);
        assert!(handler.sigint.load(Ordering::Relaxed));
        assert!(!handler.sigtstp.load(Ordering::Relaxed));
        assert!(!handler.sigterm.load(Ordering::Relaxed));
        assert!(!handler.sighup.load(Ordering::Relaxed));
        assert!(!handler.sigwinch.load(Ordering::Relaxed));
        assert!(!handler.sigchld.load(Ordering::Relaxed));
    }

    #[test]
    fn test_propagate_winsize_invalid_master_fd() {
        let handler = SignalHandler::new(-1, 1234, false);
        handler.sigwinch.store(true, Ordering::Relaxed);
        let result = handler.check_and_handle();
        assert!(result.is_ok());
    }

    #[test]
    fn test_propagate_winsize_sigwinch_flag_cleared() {
        let handler = SignalHandler::new(-1, 1234, false);
        handler.sigwinch.store(true, Ordering::Relaxed);
        let _ = handler.check_and_handle();
        assert!(!handler.sigwinch.load(Ordering::Relaxed));
    }

    #[test]
    fn test_check_and_handle_no_signals_ok() {
        let handler = SignalHandler::new(-1, 1234, false);
        let result = handler.check_and_handle();
        assert!(result.is_ok());
    }
}

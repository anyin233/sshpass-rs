use thiserror::Error;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshpassExitCode {
    Success = 0,
    InvalidArguments = 1,
    ConflictingArguments = 2,
    RuntimeError = 3,
    ParseError = 4,
    IncorrectPassword = 5,
    HostKeyUnknown = 6,
    HostKeyChanged = 7,
}

impl From<SshpassExitCode> for i32 {
    fn from(code: SshpassExitCode) -> i32 {
        code as i32
    }
}

impl From<i32> for SshpassExitCode {
    fn from(val: i32) -> SshpassExitCode {
        match val {
            0 => SshpassExitCode::Success,
            1 => SshpassExitCode::InvalidArguments,
            2 => SshpassExitCode::ConflictingArguments,
            3 => SshpassExitCode::RuntimeError,
            4 => SshpassExitCode::ParseError,
            5 => SshpassExitCode::IncorrectPassword,
            6 => SshpassExitCode::HostKeyUnknown,
            7 => SshpassExitCode::HostKeyChanged,
            _ => SshpassExitCode::RuntimeError,
        }
    }
}

#[derive(Debug, Error)]
pub enum SshpassError {
    #[error("password source error: {0}")]
    PasswordSource(String),

    #[error("PTY creation failed: {0}")]
    PtyCreation(String),

    #[error("child process spawn failed: {0}")]
    ChildSpawn(String),

    #[error("keychain access error: {0}")]
    KeychainAccess(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<&SshpassError> for SshpassExitCode {
    fn from(err: &SshpassError) -> SshpassExitCode {
        match err {
            SshpassError::PasswordSource(_) => SshpassExitCode::RuntimeError,
            SshpassError::PtyCreation(_) => SshpassExitCode::RuntimeError,
            SshpassError::ChildSpawn(_) => SshpassExitCode::RuntimeError,
            SshpassError::KeychainAccess(_) => SshpassExitCode::RuntimeError,
            SshpassError::Io(_) => SshpassExitCode::RuntimeError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_values() {
        assert_eq!(i32::from(SshpassExitCode::Success), 0);
        assert_eq!(i32::from(SshpassExitCode::InvalidArguments), 1);
        assert_eq!(i32::from(SshpassExitCode::ConflictingArguments), 2);
        assert_eq!(i32::from(SshpassExitCode::RuntimeError), 3);
        assert_eq!(i32::from(SshpassExitCode::ParseError), 4);
        assert_eq!(i32::from(SshpassExitCode::IncorrectPassword), 5);
        assert_eq!(i32::from(SshpassExitCode::HostKeyUnknown), 6);
        assert_eq!(i32::from(SshpassExitCode::HostKeyChanged), 7);
    }

    #[test]
    fn test_exit_code_from_i32() {
        assert!(matches!(SshpassExitCode::from(0), SshpassExitCode::Success));
        assert!(matches!(
            SshpassExitCode::from(1),
            SshpassExitCode::InvalidArguments
        ));
        assert!(matches!(
            SshpassExitCode::from(2),
            SshpassExitCode::ConflictingArguments
        ));
        assert!(matches!(
            SshpassExitCode::from(3),
            SshpassExitCode::RuntimeError
        ));
        assert!(matches!(
            SshpassExitCode::from(4),
            SshpassExitCode::ParseError
        ));
        assert!(matches!(
            SshpassExitCode::from(5),
            SshpassExitCode::IncorrectPassword
        ));
        assert!(matches!(
            SshpassExitCode::from(6),
            SshpassExitCode::HostKeyUnknown
        ));
        assert!(matches!(
            SshpassExitCode::from(7),
            SshpassExitCode::HostKeyChanged
        ));
        assert!(matches!(
            SshpassExitCode::from(99),
            SshpassExitCode::RuntimeError
        ));
    }

    #[test]
    fn test_error_to_exit_code() {
        let err = SshpassError::PasswordSource("bad source".to_string());
        assert!(matches!(
            SshpassExitCode::from(&err),
            SshpassExitCode::RuntimeError
        ));

        let err = SshpassError::PtyCreation("pty failed".to_string());
        assert!(matches!(
            SshpassExitCode::from(&err),
            SshpassExitCode::RuntimeError
        ));

        let err = SshpassError::ChildSpawn("spawn failed".to_string());
        assert!(matches!(
            SshpassExitCode::from(&err),
            SshpassExitCode::RuntimeError
        ));

        let err = SshpassError::KeychainAccess("keychain error".to_string());
        assert!(matches!(
            SshpassExitCode::from(&err),
            SshpassExitCode::RuntimeError
        ));

        let err = SshpassError::Io(std::io::Error::new(std::io::ErrorKind::Other, "io error"));
        assert!(matches!(
            SshpassExitCode::from(&err),
            SshpassExitCode::RuntimeError
        ));
    }

    #[test]
    fn test_error_display() {
        let err = SshpassError::PasswordSource("no env var".to_string());
        let msg = format!("{}", err);
        assert!(!msg.is_empty(), "Display message should not be empty");
        assert!(
            msg.contains("no env var"),
            "Display should include the detail: {}",
            msg
        );

        let err = SshpassError::PtyCreation("openpty failed".to_string());
        let msg = format!("{}", err);
        assert!(
            msg.contains("openpty failed"),
            "Display should include detail: {}",
            msg
        );

        let err = SshpassError::ChildSpawn("exec failed".to_string());
        let msg = format!("{}", err);
        assert!(
            msg.contains("exec failed"),
            "Display should include detail: {}",
            msg
        );

        let err = SshpassError::KeychainAccess("no entry".to_string());
        let msg = format!("{}", err);
        assert!(
            msg.contains("no entry"),
            "Display should include detail: {}",
            msg
        );
    }
}

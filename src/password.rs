use secrecy::SecretString;
use std::io::{self, BufRead};
use std::path::PathBuf;

use crate::error::SshpassError;

pub trait PasswordSource {
    fn resolve(&self) -> Result<SecretString, SshpassError>;
}

pub struct ArgumentPassword {
    password: String,
}

impl ArgumentPassword {
    pub fn new(password: String) -> Self {
        Self { password }
    }
}

impl PasswordSource for ArgumentPassword {
    fn resolve(&self) -> Result<SecretString, SshpassError> {
        Ok(SecretString::from(self.password.clone()))
    }
}

pub struct FilePassword {
    path: PathBuf,
}

impl FilePassword {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl PasswordSource for FilePassword {
    fn resolve(&self) -> Result<SecretString, SshpassError> {
        let file = std::fs::File::open(&self.path).map_err(|e| {
            SshpassError::PasswordSource(format!(
                "failed to open password file '{}': {}",
                self.path.display(),
                e
            ))
        })?;
        let mut reader = io::BufReader::new(file);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| {
            SshpassError::PasswordSource(format!(
                "failed to read password file '{}': {}",
                self.path.display(),
                e
            ))
        })?;
        let password = line
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string();
        Ok(SecretString::from(password))
    }
}

pub struct FdPassword {
    fd: i32,
}

impl FdPassword {
    pub fn new(fd: i32) -> Self {
        Self { fd }
    }
}

impl PasswordSource for FdPassword {
    fn resolve(&self) -> Result<SecretString, SshpassError> {
        use std::mem::ManuallyDrop;
        use std::os::unix::io::FromRawFd;
        let file = ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(self.fd) });
        let mut reader = io::BufReader::new(&*file);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| {
            SshpassError::PasswordSource(format!("failed to read from file descriptor: {}", e))
        })?;
        let password = line
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string();
        Ok(SecretString::from(password))
    }
}

pub struct EnvPassword;

impl PasswordSource for EnvPassword {
    fn resolve(&self) -> Result<SecretString, SshpassError> {
        let value = std::env::var("SSHPASS").map_err(|_| {
            SshpassError::PasswordSource("SSHPASS environment variable not set".to_string())
        })?;
        std::env::remove_var("SSHPASS");
        Ok(SecretString::from(value))
    }
}

pub struct StdinPassword;

impl PasswordSource for StdinPassword {
    fn resolve(&self) -> Result<SecretString, SshpassError> {
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line).map_err(|e| {
            SshpassError::PasswordSource(format!("failed to read from stdin: {}", e))
        })?;
        let password = line
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string();
        Ok(SecretString::from(password))
    }
}

pub enum PasswordResolver {
    Argument(String),
    File(PathBuf),
    FileDescriptor(i32),
    Environment,
    Keychain(String),
    Stdin,
}

impl PasswordResolver {
    pub fn resolve(&self) -> Result<SecretString, SshpassError> {
        match self {
            PasswordResolver::Argument(s) => ArgumentPassword::new(s.clone()).resolve(),
            PasswordResolver::File(path) => FilePassword::new(path.clone()).resolve(),
            PasswordResolver::FileDescriptor(fd) => FdPassword::new(*fd).resolve(),
            PasswordResolver::Environment => EnvPassword.resolve(),
            PasswordResolver::Keychain(_) => unimplemented!("not yet implemented"),
            PasswordResolver::Stdin => StdinPassword.resolve(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_argument_source() {
        let src = ArgumentPassword::new("hunter2".to_string());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "hunter2");
    }

    #[test]
    fn test_argument_uses_secret_string() {
        let src = ArgumentPassword::new("s3cr3t".to_string());
        let result: Result<SecretString, SshpassError> = src.resolve();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().expose_secret(), "s3cr3t");
    }

    #[test]
    fn test_resolver_argument() {
        let resolver = PasswordResolver::Argument("mypassword".to_string());
        let result = resolver.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "mypassword");
    }

    #[test]
    fn test_file_source() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "mypassword\n").unwrap();
        let src = FilePassword::new(f.path().to_path_buf());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "mypassword");
    }

    #[test]
    fn test_file_strips_newline() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "pass\n").unwrap();
        let src = FilePassword::new(f.path().to_path_buf());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "pass");
    }

    #[test]
    fn test_file_strips_crlf() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "pass\r\n").unwrap();
        let src = FilePassword::new(f.path().to_path_buf());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "pass");
    }

    #[test]
    fn test_file_not_found() {
        let src = FilePassword::new(PathBuf::from("/nonexistent/path/to/password.txt"));
        let result = src.resolve();
        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::PasswordSource(_) => {}
            e => panic!("expected PasswordSource error, got {:?}", e),
        }
    }

    #[test]
    fn test_file_empty() {
        let f = NamedTempFile::new().unwrap();
        let src = FilePassword::new(f.path().to_path_buf());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "");
    }

    #[test]
    fn test_resolver_file() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "filepass\n").unwrap();
        let resolver = PasswordResolver::File(f.path().to_path_buf());
        let result = resolver.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "filepass");
    }

    #[test]
    fn test_fd_source() {
        use std::os::unix::io::IntoRawFd;
        let (read_fd, write_fd) = nix::unistd::pipe().expect("pipe failed");
        {
            use std::os::unix::io::FromRawFd;
            let mut write_file = unsafe { std::fs::File::from_raw_fd(write_fd.into_raw_fd()) };
            write!(write_file, "testpass\n").unwrap();
        }
        let src = FdPassword::new(read_fd.into_raw_fd());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "testpass");
    }

    #[test]
    fn test_fd_strips_newline() {
        use std::os::unix::io::IntoRawFd;
        let (read_fd, write_fd) = nix::unistd::pipe().expect("pipe failed");
        {
            use std::os::unix::io::FromRawFd;
            let mut write_file = unsafe { std::fs::File::from_raw_fd(write_fd.into_raw_fd()) };
            write!(write_file, "mypass\n").unwrap();
        }
        let src = FdPassword::new(read_fd.into_raw_fd());
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "mypass");
    }

    #[test]
    fn test_env_source() {
        std::env::set_var("SSHPASS", "envpassword");
        let src = EnvPassword;
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "envpassword");
    }

    #[test]
    fn test_env_cleanup() {
        std::env::set_var("SSHPASS", "cleanup_test");
        let src = EnvPassword;
        src.resolve().expect("should resolve");
        assert!(
            std::env::var("SSHPASS").is_err(),
            "SSHPASS should be removed after resolve"
        );
    }

    #[test]
    fn test_env_not_set() {
        std::env::remove_var("SSHPASS");
        let src = EnvPassword;
        let result = src.resolve();
        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::PasswordSource(_) => {}
            e => panic!("expected PasswordSource error, got {:?}", e),
        }
    }

    #[test]
    fn test_resolver_environment() {
        std::env::set_var("SSHPASS", "resolver_env_pass");
        let resolver = PasswordResolver::Environment;
        let result = resolver.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "resolver_env_pass");
    }

    #[test]
    fn test_resolver_fd() {
        use std::os::unix::io::IntoRawFd;
        let (read_fd, write_fd) = nix::unistd::pipe().expect("pipe failed");
        {
            use std::os::unix::io::FromRawFd;
            let mut write_file = unsafe { std::fs::File::from_raw_fd(write_fd.into_raw_fd()) };
            write!(write_file, "fdpass\n").unwrap();
        }
        let resolver = PasswordResolver::FileDescriptor(read_fd.into_raw_fd());
        let result = resolver.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "fdpass");
    }
}

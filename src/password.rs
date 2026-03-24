use secrecy::SecretString;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::rc::Rc;

use crate::error::SshpassError;
use crate::keychain::KeychainBackend;

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

pub struct KeychainPassword {
    key: String,
    backend: Rc<dyn KeychainBackend>,
}

impl KeychainPassword {
    pub fn new(key: String, backend: Box<dyn KeychainBackend>) -> Self {
        Self {
            key,
            backend: Rc::from(backend),
        }
    }

    #[allow(dead_code)]
    pub fn new_with_shared_backend(key: String, backend: Rc<dyn KeychainBackend>) -> Self {
        Self { key, backend }
    }

    fn prompt_and_maybe_save(&self) -> Result<SecretString, SshpassError> {
        eprintln!("No password found for key '{}' in Keychain.", self.key);

        let password = match std::env::var("SSHPASS_RS_TEST_PASSWORD") {
            Ok(test_pw) => test_pw,
            Err(_) => rpassword::prompt_password("Enter password: ").map_err(|e| {
                SshpassError::PasswordSource(format!("failed to read password: {}", e))
            })?,
        };

        let should_save = match std::env::var("SSHPASS_RS_TEST_SAVE") {
            Ok(val) => val == "1",
            Err(_) => {
                eprint!("Save to Keychain? [Y/n]: ");
                let mut input = String::new();
                io::stdin().lock().read_line(&mut input).map_err(|e| {
                    SshpassError::PasswordSource(format!("failed to read input: {}", e))
                })?;
                let trimmed = input.trim().to_lowercase();
                trimmed.is_empty() || trimmed == "y" || trimmed == "yes"
            }
        };

        if should_save {
            let secret = SecretString::from(password.clone());
            self.backend.store(&self.key, &secret)?;
        }

        Ok(SecretString::from(password))
    }
}

impl PasswordSource for KeychainPassword {
    fn resolve(&self) -> Result<SecretString, SshpassError> {
        match self.backend.get(&self.key) {
            Ok(password) => Ok(password),
            Err(SshpassError::KeychainAccess(ref msg)) if msg.starts_with("key not found:") => {
                self.prompt_and_maybe_save()
            }
            Err(e) => Err(e),
        }
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

    pub fn resolve_with_keychain(
        &self,
        backend: Box<dyn KeychainBackend>,
    ) -> Result<SecretString, SshpassError> {
        match self {
            PasswordResolver::Keychain(key) => {
                KeychainPassword::new(key.clone(), backend).resolve()
            }
            _ => self.resolve(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keychain::InMemoryKeychainBackend;
    use secrecy::ExposeSecret;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::NamedTempFile;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

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
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SSHPASS", "envpassword");
        let src = EnvPassword;
        let result = src.resolve().expect("should resolve");
        assert_eq!(result.expose_secret(), "envpassword");
    }

    #[test]
    fn test_env_cleanup() {
        let _lock = ENV_MUTEX.lock().unwrap();
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
        let _lock = ENV_MUTEX.lock().unwrap();
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
        let _lock = ENV_MUTEX.lock().unwrap();
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

    #[test]
    fn test_keychain_hit() {
        let backend = InMemoryKeychainBackend::new();
        backend
            .store("myserver", &SecretString::from("stored_pass"))
            .unwrap();

        let src = KeychainPassword::new("myserver".to_string(), Box::new(backend));
        let result = src.resolve().expect("should resolve from keychain");
        assert_eq!(result.expose_secret(), "stored_pass");
    }

    #[test]
    fn test_keychain_miss_with_test_password() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let backend = InMemoryKeychainBackend::new();

        std::env::set_var("SSHPASS_RS_TEST_PASSWORD", "fallback_pass");
        std::env::set_var("SSHPASS_RS_TEST_SAVE", "0");

        let src = KeychainPassword::new("missing_key".to_string(), Box::new(backend));
        let result = src.resolve().expect("should resolve via test env var");
        assert_eq!(result.expose_secret(), "fallback_pass");

        std::env::remove_var("SSHPASS_RS_TEST_PASSWORD");
        std::env::remove_var("SSHPASS_RS_TEST_SAVE");
    }

    #[test]
    fn test_keychain_miss_with_save() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let backend = InMemoryKeychainBackend::new();

        std::env::set_var("SSHPASS_RS_TEST_PASSWORD", "save_me_pass");
        std::env::set_var("SSHPASS_RS_TEST_SAVE", "1");

        let src = KeychainPassword::new("save_key".to_string(), Box::new(backend));
        let result = src.resolve().expect("should resolve and save");
        assert_eq!(result.expose_secret(), "save_me_pass");

        std::env::remove_var("SSHPASS_RS_TEST_PASSWORD");
        std::env::remove_var("SSHPASS_RS_TEST_SAVE");
    }

    #[test]
    fn test_keychain_miss_no_save() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let backend = InMemoryKeychainBackend::new();

        std::env::set_var("SSHPASS_RS_TEST_PASSWORD", "nosave_pass");
        std::env::set_var("SSHPASS_RS_TEST_SAVE", "0");

        let src = KeychainPassword::new("nosave_key".to_string(), Box::new(backend));
        let result = src.resolve().expect("should resolve without saving");
        assert_eq!(result.expose_secret(), "nosave_pass");

        std::env::remove_var("SSHPASS_RS_TEST_PASSWORD");
        std::env::remove_var("SSHPASS_RS_TEST_SAVE");
    }

    #[test]
    fn test_keychain_miss_save_verifies_stored() {
        let _lock = ENV_MUTEX.lock().unwrap();
        use std::rc::Rc;

        let backend = Rc::new(InMemoryKeychainBackend::new());
        let backend_clone = Rc::clone(&backend);

        std::env::set_var("SSHPASS_RS_TEST_PASSWORD", "verify_stored");
        std::env::set_var("SSHPASS_RS_TEST_SAVE", "1");

        let src =
            KeychainPassword::new_with_shared_backend("verify_key".to_string(), backend_clone);
        let result = src.resolve().expect("should resolve and save");
        assert_eq!(result.expose_secret(), "verify_stored");

        let stored = backend.get("verify_key").expect("should be stored");
        assert_eq!(stored.expose_secret(), "verify_stored");

        std::env::remove_var("SSHPASS_RS_TEST_PASSWORD");
        std::env::remove_var("SSHPASS_RS_TEST_SAVE");
    }

    #[test]
    fn test_keychain_miss_nosave_verifies_not_stored() {
        let _lock = ENV_MUTEX.lock().unwrap();
        use std::rc::Rc;

        let backend = Rc::new(InMemoryKeychainBackend::new());
        let backend_clone = Rc::clone(&backend);

        std::env::set_var("SSHPASS_RS_TEST_PASSWORD", "dont_store_me");
        std::env::set_var("SSHPASS_RS_TEST_SAVE", "0");

        let src =
            KeychainPassword::new_with_shared_backend("nostore_key".to_string(), backend_clone);
        let result = src.resolve().expect("should resolve without saving");
        assert_eq!(result.expose_secret(), "dont_store_me");

        let stored = backend.get("nostore_key");
        assert!(stored.is_err(), "password should NOT be stored in backend");

        std::env::remove_var("SSHPASS_RS_TEST_PASSWORD");
        std::env::remove_var("SSHPASS_RS_TEST_SAVE");
    }

    /// A mock backend that always returns an operational error (not "key not found").
    /// Used to verify that `KeychainPassword::resolve()` propagates backend failures
    /// instead of silently falling back to prompting.
    struct FailingKeychainBackend;

    impl KeychainBackend for FailingKeychainBackend {
        fn store(&self, _key: &str, _password: &SecretString) -> Result<(), SshpassError> {
            Err(SshpassError::KeychainAccess(
                "1Password CLI (op) not found".to_string(),
            ))
        }

        fn get(&self, _key: &str) -> Result<SecretString, SshpassError> {
            Err(SshpassError::KeychainAccess(
                "1Password CLI (op) not found".to_string(),
            ))
        }

        fn delete(&self, _key: &str) -> Result<(), SshpassError> {
            Err(SshpassError::KeychainAccess(
                "1Password CLI (op) not found".to_string(),
            ))
        }

        fn list(&self) -> Result<Vec<String>, SshpassError> {
            Err(SshpassError::KeychainAccess(
                "1Password CLI (op) not found".to_string(),
            ))
        }
    }

    #[test]
    fn test_keychain_backend_failure_propagates() {
        let backend = FailingKeychainBackend;
        let src = KeychainPassword::new("any_key".to_string(), Box::new(backend));

        let result = src.resolve();
        assert!(
            result.is_err(),
            "operational error should propagate, not fallback to prompt"
        );

        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert_eq!(msg, "1Password CLI (op) not found");
            }
            other => panic!("expected KeychainAccess error, got: {}", other),
        }
    }
}

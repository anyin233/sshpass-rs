use secrecy::SecretString;
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
            PasswordResolver::File(_) => unimplemented!("not yet implemented"),
            PasswordResolver::FileDescriptor(_) => unimplemented!("not yet implemented"),
            PasswordResolver::Environment => unimplemented!("not yet implemented"),
            PasswordResolver::Keychain(_) => unimplemented!("not yet implemented"),
            PasswordResolver::Stdin => unimplemented!("not yet implemented"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

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
}

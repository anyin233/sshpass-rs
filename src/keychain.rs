use crate::error::SshpassError;
use secrecy::{ExposeSecret, SecretString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const SERVICE_NAME: &str = "sshpass-rs";

pub trait KeychainBackend {
    fn store(&self, key: &str, password: &SecretString) -> Result<(), SshpassError>;
    fn get(&self, key: &str) -> Result<SecretString, SshpassError>;
    fn delete(&self, key: &str) -> Result<(), SshpassError>;
    fn list(&self) -> Result<Vec<String>, SshpassError>;
}

pub struct RealKeychainBackend;

impl RealKeychainBackend {
    const INDEX_KEY: &'static str = "__sshpass_rs_index__";

    fn entry(key: &str) -> Result<keyring::Entry, SshpassError> {
        keyring::Entry::new(SERVICE_NAME, key)
            .map_err(|e| SshpassError::KeychainAccess(format!("failed to create entry: {}", e)))
    }

    fn read_index(&self) -> Result<Vec<String>, SshpassError> {
        match Self::entry(Self::INDEX_KEY)?.get_password() {
            Ok(data) => Ok(data
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()),
            Err(keyring::Error::NoEntry) => Ok(Vec::new()),
            Err(e) => Err(SshpassError::KeychainAccess(format!(
                "failed to read index: {}",
                e
            ))),
        }
    }

    fn write_index(&self, keys: &[String]) -> Result<(), SshpassError> {
        let data = keys.join("\n");
        Self::entry(Self::INDEX_KEY)?
            .set_password(&data)
            .map_err(|e| SshpassError::KeychainAccess(format!("failed to write index: {}", e)))
    }
}

impl KeychainBackend for RealKeychainBackend {
    fn store(&self, key: &str, password: &SecretString) -> Result<(), SshpassError> {
        Self::entry(key)?
            .set_password(password.expose_secret())
            .map_err(|e| SshpassError::KeychainAccess(format!("failed to store: {}", e)))?;

        let mut index = self.read_index()?;
        if !index.contains(&key.to_string()) {
            index.push(key.to_string());
            self.write_index(&index)?;
        }
        Ok(())
    }

    fn get(&self, key: &str) -> Result<SecretString, SshpassError> {
        match Self::entry(key)?.get_password() {
            Ok(pw) => Ok(SecretString::from(pw)),
            Err(keyring::Error::NoEntry) => Err(SshpassError::KeychainAccess(format!(
                "key not found: {}",
                key
            ))),
            Err(e) => Err(SshpassError::KeychainAccess(format!(
                "failed to get: {}",
                e
            ))),
        }
    }

    fn delete(&self, key: &str) -> Result<(), SshpassError> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) => {}
            Err(keyring::Error::NoEntry) => {
                return Err(SshpassError::KeychainAccess(format!(
                    "key not found: {}",
                    key
                )));
            }
            Err(e) => {
                return Err(SshpassError::KeychainAccess(format!(
                    "failed to delete: {}",
                    e
                )));
            }
        }

        let mut index = self.read_index()?;
        index.retain(|k| k != key);
        self.write_index(&index)
    }

    fn list(&self) -> Result<Vec<String>, SshpassError> {
        self.read_index()
    }
}

#[allow(dead_code)]
pub struct InMemoryKeychainBackend {
    store: RefCell<HashMap<String, String>>,
}

impl InMemoryKeychainBackend {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            store: RefCell::new(HashMap::new()),
        }
    }
}

impl KeychainBackend for InMemoryKeychainBackend {
    fn store(&self, key: &str, password: &SecretString) -> Result<(), SshpassError> {
        self.store
            .borrow_mut()
            .insert(key.to_string(), password.expose_secret().to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<SecretString, SshpassError> {
        self.store
            .borrow()
            .get(key)
            .map(|v| SecretString::from(v.clone()))
            .ok_or_else(|| SshpassError::KeychainAccess(format!("key not found: {}", key)))
    }

    fn delete(&self, key: &str) -> Result<(), SshpassError> {
        self.store
            .borrow_mut()
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| SshpassError::KeychainAccess(format!("key not found: {}", key)))
    }

    fn list(&self) -> Result<Vec<String>, SshpassError> {
        Ok(self.store.borrow().keys().cloned().collect())
    }
}

pub struct FileKeychainBackend {
    path: PathBuf,
}

impl FileKeychainBackend {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn read_store(&self) -> Result<(HashMap<String, String>, Vec<String>), SshpassError> {
        if !self.path.exists() {
            return Ok((HashMap::new(), Vec::new()));
        }
        let content = fs::read_to_string(&self.path)?;
        Self::parse_json(&content)
    }

    fn write_store(
        &self,
        passwords: &HashMap<String, String>,
        index: &[String],
    ) -> Result<(), SshpassError> {
        let json = Self::to_json(passwords, index);
        let dir = self
            .path
            .parent()
            .ok_or_else(|| SshpassError::KeychainAccess("invalid path".to_string()))?;

        let tmp_path = dir.join(format!(".sshpass_tmp_{}", std::process::id()));
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    fn parse_json(content: &str) -> Result<(HashMap<String, String>, Vec<String>), SshpassError> {
        let content = content.trim();
        if content.is_empty() || content == "{}" {
            return Ok((HashMap::new(), Vec::new()));
        }

        let mut passwords = HashMap::new();
        let mut index = Vec::new();

        let passwords_start = content
            .find("\"passwords\"")
            .and_then(|i| content[i..].find('{').map(|j| i + j));
        if let Some(start) = passwords_start {
            let end = content[start..]
                .find('}')
                .map(|i| start + i)
                .unwrap_or(content.len());
            let inner = &content[start + 1..end];
            for pair in Self::split_json_pairs(inner) {
                if let Some((k, v)) = Self::parse_kv_pair(pair.trim()) {
                    passwords.insert(k, v);
                }
            }
        }

        let index_start = content
            .find("\"index\"")
            .and_then(|i| content[i..].find('[').map(|j| i + j));
        if let Some(start) = index_start {
            let end = content[start..]
                .find(']')
                .map(|i| start + i)
                .unwrap_or(content.len());
            let inner = &content[start + 1..end];
            for item in inner.split(',') {
                let item = item.trim().trim_matches('"');
                if !item.is_empty() {
                    index.push(item.to_string());
                }
            }
        }

        Ok((passwords, index))
    }

    fn split_json_pairs(inner: &str) -> Vec<&str> {
        let mut pairs = Vec::new();
        let mut depth = 0;
        let mut start = 0;
        let mut in_string = false;
        let mut prev_escape = false;

        for (i, ch) in inner.char_indices() {
            if prev_escape {
                prev_escape = false;
                continue;
            }
            match ch {
                '\\' if in_string => prev_escape = true,
                '"' => in_string = !in_string,
                '{' | '[' if !in_string => depth += 1,
                '}' | ']' if !in_string => depth -= 1,
                ',' if !in_string && depth == 0 => {
                    pairs.push(&inner[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        if start < inner.len() {
            pairs.push(&inner[start..]);
        }
        pairs
    }

    fn parse_kv_pair(pair: &str) -> Option<(String, String)> {
        let colon = pair.find(':')?;
        let key = pair[..colon].trim().trim_matches('"');
        let val = pair[colon + 1..].trim().trim_matches('"');
        if key.is_empty() {
            return None;
        }
        Some((key.to_string(), val.to_string()))
    }

    fn to_json(passwords: &HashMap<String, String>, index: &[String]) -> String {
        let mut pw_entries: Vec<String> = passwords
            .iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
            .collect();
        pw_entries.sort();

        let idx_entries: Vec<String> = index.iter().map(|k| format!("\"{}\"", k)).collect();

        format!(
            "{{\"passwords\":{{{}}},\"index\":[{}]}}",
            pw_entries.join(","),
            idx_entries.join(",")
        )
    }
}

impl KeychainBackend for FileKeychainBackend {
    fn store(&self, key: &str, password: &SecretString) -> Result<(), SshpassError> {
        let (mut passwords, mut index) = self.read_store()?;
        passwords.insert(key.to_string(), password.expose_secret().to_string());
        if !index.contains(&key.to_string()) {
            index.push(key.to_string());
        }
        self.write_store(&passwords, &index)
    }

    fn get(&self, key: &str) -> Result<SecretString, SshpassError> {
        let (passwords, _) = self.read_store()?;
        passwords
            .get(key)
            .map(|v| SecretString::from(v.clone()))
            .ok_or_else(|| SshpassError::KeychainAccess(format!("key not found: {}", key)))
    }

    fn delete(&self, key: &str) -> Result<(), SshpassError> {
        let (mut passwords, mut index) = self.read_store()?;
        if passwords.remove(key).is_none() {
            return Err(SshpassError::KeychainAccess(format!(
                "key not found: {}",
                key
            )));
        }
        index.retain(|k| k != key);
        self.write_store(&passwords, &index)
    }

    fn list(&self) -> Result<Vec<String>, SshpassError> {
        let (_, index) = self.read_store()?;
        Ok(index)
    }
}

pub fn handle_store(manager: &KeychainManager, key: &str) -> Result<(), SshpassError> {
    let password = match std::env::var("SSHPASS_RS_TEST_PASSWORD") {
        Ok(p) => p,
        Err(_) => {
            rpassword::prompt_password("Enter password to store: ").map_err(SshpassError::Io)?
        }
    };
    let secret = SecretString::from(password);
    manager.store(key, &secret)?;
    println!("Password stored for key '{key}'");
    Ok(())
}

pub fn handle_delete(manager: &KeychainManager, key: &str) -> Result<(), SshpassError> {
    manager.delete(key)?;
    println!("Password deleted for key '{key}'");
    Ok(())
}

pub fn handle_list(manager: &KeychainManager) -> Result<(), SshpassError> {
    let keys = manager.list()?;
    if keys.is_empty() {
        println!("(empty)");
    } else {
        for key in &keys {
            println!("{key}");
        }
    }
    Ok(())
}

pub struct KeychainManager {
    backend: Box<dyn KeychainBackend>,
}

impl KeychainManager {
    pub fn new(backend: Box<dyn KeychainBackend>) -> Self {
        Self { backend }
    }

    #[allow(dead_code)]
    pub fn from_env() -> Self {
        match std::env::var("SSHPASS_RS_TEST_KEYCHAIN_FILE") {
            Ok(path) => Self::new(Box::new(FileKeychainBackend::new(path))),
            Err(_) => Self::new(Box::new(RealKeychainBackend)),
        }
    }

    pub fn store(&self, key: &str, password: &SecretString) -> Result<(), SshpassError> {
        self.backend.store(key, password)
    }

    #[allow(dead_code)]
    pub fn get(&self, key: &str) -> Result<SecretString, SshpassError> {
        self.backend.get(key)
    }

    pub fn delete(&self, key: &str) -> Result<(), SshpassError> {
        self.backend.delete(key)
    }

    pub fn list(&self) -> Result<Vec<String>, SshpassError> {
        self.backend.list()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inmemory_store_and_get() {
        let backend = InMemoryKeychainBackend::new();
        let password = SecretString::from("s3cret");

        backend.store("myhost", &password).unwrap();
        let retrieved = backend.get("myhost").unwrap();

        assert_eq!(retrieved.expose_secret(), "s3cret");
    }

    #[test]
    fn test_inmemory_delete() {
        let backend = InMemoryKeychainBackend::new();
        let password = SecretString::from("s3cret");

        backend.store("myhost", &password).unwrap();
        backend.delete("myhost").unwrap();

        let result = backend.get("myhost");
        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(_) => {}
            other => panic!("Expected KeychainAccess, got: {:?}", other),
        }
    }

    #[test]
    fn test_inmemory_list() {
        let backend = InMemoryKeychainBackend::new();

        backend
            .store("host1", &SecretString::from("pass1"))
            .unwrap();
        backend
            .store("host2", &SecretString::from("pass2"))
            .unwrap();
        backend
            .store("host3", &SecretString::from("pass3"))
            .unwrap();

        let mut keys = backend.list().unwrap();
        keys.sort();

        assert_eq!(keys, vec!["host1", "host2", "host3"]);
    }

    #[test]
    fn test_inmemory_list_empty() {
        let backend = InMemoryKeychainBackend::new();
        let keys = backend.list().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_inmemory_get_nonexistent() {
        let backend = InMemoryKeychainBackend::new();
        let result = backend.get("nonexistent");

        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("nonexistent"),
                    "Error should mention the key: {}",
                    msg
                );
            }
            other => panic!("Expected KeychainAccess, got: {:?}", other),
        }
    }

    #[test]
    fn test_file_backend_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("keychain.json");

        {
            let backend = FileKeychainBackend::new(&file_path);
            let password = SecretString::from("persistent_pass");
            backend.store("persist_key", &password).unwrap();
        }

        {
            let backend = FileKeychainBackend::new(&file_path);
            let retrieved = backend.get("persist_key").unwrap();
            assert_eq!(retrieved.expose_secret(), "persistent_pass");
        }
    }

    #[test]
    fn test_file_backend_list() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("keychain.json");

        let backend = FileKeychainBackend::new(&file_path);
        backend
            .store("alpha", &SecretString::from("pass_a"))
            .unwrap();
        backend
            .store("beta", &SecretString::from("pass_b"))
            .unwrap();

        let mut keys = backend.list().unwrap();
        keys.sort();

        assert_eq!(keys, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_manager_from_env_with_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("env_keychain.json");

        std::env::set_var("SSHPASS_RS_TEST_KEYCHAIN_FILE", file_path.to_str().unwrap());

        let manager = KeychainManager::from_env();
        let password = SecretString::from("env_pass");
        manager.store("env_key", &password).unwrap();

        let retrieved = manager.get("env_key").unwrap();
        assert_eq!(retrieved.expose_secret(), "env_pass");

        std::env::remove_var("SSHPASS_RS_TEST_KEYCHAIN_FILE");
    }

    #[test]
    fn test_store_handler() {
        std::env::set_var("SSHPASS_RS_TEST_PASSWORD", "handler_pass");

        let backend = InMemoryKeychainBackend::new();
        let manager = KeychainManager::new(Box::new(backend));

        handle_store(&manager, "test_key").unwrap();

        let retrieved = manager.get("test_key").unwrap();
        assert_eq!(retrieved.expose_secret(), "handler_pass");

        std::env::remove_var("SSHPASS_RS_TEST_PASSWORD");
    }

    #[test]
    fn test_delete_handler() {
        let backend = InMemoryKeychainBackend::new();
        let manager = KeychainManager::new(Box::new(backend));

        manager
            .store("del_key", &SecretString::from("some_pass"))
            .unwrap();

        handle_delete(&manager, "del_key").unwrap();

        let result = manager.get("del_key");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_handler() {
        let backend = InMemoryKeychainBackend::new();
        let manager = KeychainManager::new(Box::new(backend));

        manager
            .store("key_a", &SecretString::from("pass_a"))
            .unwrap();
        manager
            .store("key_b", &SecretString::from("pass_b"))
            .unwrap();

        handle_list(&manager).unwrap();

        let mut keys = manager.list().unwrap();
        keys.sort();
        assert_eq!(keys, vec!["key_a", "key_b"]);
    }

    #[test]
    fn test_list_empty() {
        let backend = InMemoryKeychainBackend::new();
        let manager = KeychainManager::new(Box::new(backend));

        handle_list(&manager).unwrap();

        let keys = manager.list().unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_delete_nonexistent() {
        let backend = InMemoryKeychainBackend::new();
        let manager = KeychainManager::new(Box::new(backend));

        let result = handle_delete(&manager, "nonexistent_key");
        assert!(result.is_err());
    }
}

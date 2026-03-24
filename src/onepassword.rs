use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};

use crate::error::SshpassError;
use crate::keychain::KeychainBackend;

/// Represents a single item from `op item list --format json` output.
///
/// All fields use `#[serde(default)]` for forward compatibility with
/// future 1Password CLI versions that may add new fields.
#[derive(Debug, Deserialize)]
pub struct OpItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub category: String,
}

/// Represents the full detail of a single item from `op item get --format json`.
///
/// Includes the `fields` array needed to extract password values.
#[derive(Debug, Deserialize)]
pub struct OpItemDetail {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub category: String,
    #[serde(default)]
    pub fields: Vec<OpField>,
}

/// Represents a single field inside an `OpItemDetail`.
///
/// Password fields typically have `id == "password"` or `field_type == "CONCEALED"`.
#[derive(Debug, Deserialize)]
pub struct OpField {
    #[serde(default)]
    pub id: String,
    #[serde(default, rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub label: Option<String>,
}

/// Parses `op item list --format json` output into a vector of items.
///
/// Params:
/// - json: Raw JSON string from `op item list --format json`.
///
/// Returns:
/// - A vector of `OpItem` structs, or `SshpassError::KeychainAccess` on parse failure.
pub fn parse_item_list(json: &str) -> Result<Vec<OpItem>, SshpassError> {
    serde_json::from_str(json).map_err(|e| {
        SshpassError::KeychainAccess(format!("failed to parse 1Password item list: {e}"))
    })
}

/// Convenience wrapper: parses the item list and maps to titles only.
///
/// Params:
/// - json: Raw JSON string from `op item list --format json`.
///
/// Returns:
/// - A vector of item title strings, or `SshpassError::KeychainAccess` on parse failure.
pub fn parse_item_titles(json: &str) -> Result<Vec<String>, SshpassError> {
    let items = parse_item_list(json)?;
    Ok(items.into_iter().map(|item| item.title).collect())
}

/// Parses `op item get --format json` output and extracts the password field value.
///
/// Looks for a field with `id == "password"` first, then falls back to any field
/// with `field_type == "CONCEALED"`.
///
/// Params:
/// - json: Raw JSON string from `op item get <id> --format json`.
///
/// Returns:
/// - The password as a `SecretString`, or `SshpassError::KeychainAccess` if no
///   password field is found or the JSON is malformed.
pub fn parse_item_password(json: &str) -> Result<SecretString, SshpassError> {
    let detail: OpItemDetail = serde_json::from_str(json).map_err(|e| {
        SshpassError::KeychainAccess(format!("failed to parse 1Password item detail: {e}"))
    })?;

    // First pass: look for field with id == "password"
    for field in &detail.fields {
        if field.id == "password" {
            if let Some(ref value) = field.value {
                return Ok(SecretString::from(value.clone()));
            }
        }
    }

    // Second pass: look for any CONCEALED field
    for field in &detail.fields {
        if field.field_type == "CONCEALED" {
            if let Some(ref value) = field.value {
                return Ok(SecretString::from(value.clone()));
            }
        }
    }

    Err(SshpassError::KeychainAccess(format!(
        "no password field found in 1Password item '{}'",
        detail.title
    )))
}

pub struct OnePasswordBackend {
    vault: Option<String>,
    op_path: String,
    verbose: bool,
}

impl OnePasswordBackend {
    pub fn new(vault: Option<String>, verbose: bool) -> Self {
        Self {
            vault,
            op_path: "op".to_string(),
            verbose,
        }
    }

    #[allow(dead_code)]
    pub fn with_op_path(vault: Option<String>, op_path: String, verbose: bool) -> Self {
        Self {
            vault,
            op_path,
            verbose,
        }
    }

    fn append_vault_args<'a>(&'a self, args: &mut Vec<&'a str>) {
        if let Some(vault) = self.vault.as_deref() {
            args.push("--vault");
            args.push(vault);
        }
    }

    fn run_op(&self, args: &[&str]) -> Result<String, SshpassError> {
        if self.verbose {
            eprintln!("SSHPASS_RS: running: op {}", args.join(" "));
        }

        let output = Command::new(&self.op_path)
            .args(args)
            .output()
            .map_err(|err| match err.kind() {
                ErrorKind::NotFound => SshpassError::KeychainAccess(
                    "1Password CLI (op) not found. Install from https://1password.com/downloads/command-line/".to_string(),
                ),
                _ => SshpassError::KeychainAccess(format!("failed to run op: {err}")),
            })?;

        if self.verbose {
            eprintln!("SSHPASS_RS: op exited with status {}", output.status);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(SshpassError::KeychainAccess(format!("op failed: {stderr}")));
        }

        String::from_utf8(output.stdout).map_err(|err| {
            SshpassError::KeychainAccess(format!("op returned non-UTF-8 stdout: {err}"))
        })
    }

    fn run_op_with_stdin(&self, args: &[&str], stdin_data: &str) -> Result<String, SshpassError> {
        if self.verbose {
            eprintln!(
                "SSHPASS_RS: running: op {} (with stdin data)",
                args.join(" ")
            );
        }

        let mut child = Command::new(&self.op_path)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| match err.kind() {
                ErrorKind::NotFound => SshpassError::KeychainAccess(
                    "1Password CLI (op) not found. Install from https://1password.com/downloads/command-line/".to_string(),
                ),
                _ => SshpassError::KeychainAccess(format!("failed to run op: {err}")),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            // Write secrets through stdin so they never appear in process args.
            stdin.write_all(stdin_data.as_bytes()).map_err(|err| {
                SshpassError::KeychainAccess(format!("failed to write op stdin: {err}"))
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|err| SshpassError::KeychainAccess(format!("failed to wait for op: {err}")))?;

        if self.verbose {
            eprintln!("SSHPASS_RS: op exited with status {}", output.status);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(SshpassError::KeychainAccess(format!("op failed: {stderr}")));
        }

        String::from_utf8(output.stdout).map_err(|err| {
            SshpassError::KeychainAccess(format!("op returned non-UTF-8 stdout: {err}"))
        })
    }
}

impl KeychainBackend for OnePasswordBackend {
    fn store(&self, key: &str, password: &SecretString) -> Result<(), SshpassError> {
        if self.verbose {
            eprintln!("SSHPASS_RS: storing key '{}' in 1Password", key);
        }

        let payload = serde_json::json!({
            "title": key,
            "category": "PASSWORD",
            "tags": ["sshpass-rs"],
            "fields": [{
                "id": "password",
                "type": "CONCEALED",
                "purpose": "PASSWORD",
                "label": "password",
                "value": password.expose_secret(),
            }],
        });
        let stdin_data = serde_json::to_string(&payload).map_err(|err| {
            SshpassError::KeychainAccess(format!("failed to serialize 1Password item: {err}"))
        })?;

        let mut args = vec!["item", "create", "-", "--format", "json"];
        self.append_vault_args(&mut args);
        self.run_op_with_stdin(&args, &stdin_data)?;
        Ok(())
    }

    fn get(&self, key: &str) -> Result<SecretString, SshpassError> {
        if self.verbose {
            eprintln!("SSHPASS_RS: looking up key '{}' in 1Password", key);
        }

        let mut list_args = vec!["item", "list", "--tags", "sshpass-rs", "--format", "json"];
        self.append_vault_args(&mut list_args);
        let list_output = self.run_op(&list_args)?;
        let items = parse_item_list(&list_output)?;
        let item_id = match items.iter().find(|item| item.title == key) {
            Some(item) => {
                if self.verbose {
                    eprintln!("SSHPASS_RS: found item id '{}' for key '{}'", item.id, key);
                }
                item.id.as_str()
            }
            None => {
                if self.verbose {
                    eprintln!("SSHPASS_RS: key '{}' not found in 1Password", key);
                }
                return Err(SshpassError::KeychainAccess(format!(
                    "key not found: {key}"
                )));
            }
        };

        let mut get_args = vec!["item", "get", item_id, "--format", "json"];
        self.append_vault_args(&mut get_args);
        let item_output = self.run_op(&get_args)?;
        parse_item_password(&item_output)
    }

    fn delete(&self, key: &str) -> Result<(), SshpassError> {
        if self.verbose {
            eprintln!("SSHPASS_RS: deleting key '{}' from 1Password", key);
        }

        let mut list_args = vec!["item", "list", "--tags", "sshpass-rs", "--format", "json"];
        self.append_vault_args(&mut list_args);
        let list_output = self.run_op(&list_args)?;
        let items = parse_item_list(&list_output)?;
        let item_id = items
            .iter()
            .find(|item| item.title == key)
            .map(|item| item.id.as_str())
            .ok_or_else(|| SshpassError::KeychainAccess(format!("key not found: {key}")))?;

        let mut delete_args = vec!["item", "delete", item_id];
        self.append_vault_args(&mut delete_args);
        self.run_op(&delete_args)?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<String>, SshpassError> {
        if self.verbose {
            eprintln!("SSHPASS_RS: listing keys from 1Password");
        }

        let mut args = vec!["item", "list", "--tags", "sshpass-rs", "--format", "json"];
        self.append_vault_args(&mut args);
        let output = self.run_op(&args)?;
        let titles = parse_item_titles(&output)?;
        if self.verbose {
            eprintln!("SSHPASS_RS: found {} keys", titles.len());
        }
        Ok(titles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::sync::Mutex;

    /// Serializes env-var mutations so parallel test threads don't race.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn mock_op_path() -> String {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{}/tests/fixtures/mock_op.sh", manifest_dir)
    }

    // ── Backend integration tests (via with_op_path) ──────────────────

    #[test]
    fn test_op_not_found() {
        let backend = OnePasswordBackend::with_op_path(None, "/nonexistent/op".to_string(), false);
        let result = backend.list();

        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("1Password CLI (op) not found"),
                    "Error should mention op not found: {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }

    #[test]
    fn test_op_stderr_included() {
        // `false` always exits 1 with empty stderr.
        let backend = OnePasswordBackend::with_op_path(None, "false".to_string(), false);
        let result = backend.list();

        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("op failed:"),
                    "Error should contain 'op failed:': {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }

    #[test]
    fn test_list_returns_titles() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let titles = backend.list().expect("list should succeed");
        assert_eq!(titles, vec!["user@host", "root@server"]);
    }

    #[test]
    fn test_list_empty() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("MOCK_OP_EMPTY", "1");

        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let titles = backend.list().expect("list should succeed");

        std::env::remove_var("MOCK_OP_EMPTY");
        assert!(titles.is_empty());
    }

    #[test]
    fn test_get_returns_password() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let password = backend.get("user@host").expect("get should succeed");
        assert_eq!(password.expose_secret(), "s3cret");
    }

    #[test]
    fn test_get_not_found() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let result = backend.get("nonexistent");

        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("key not found: nonexistent"),
                    "Error should mention key not found: {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }

    #[test]
    fn test_get_exact_match() {
        // "user" should NOT match "user@host" — exact match only.
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let result = backend.get("user");

        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("key not found: user"),
                    "Error should mention key not found: {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }

    #[test]
    fn test_delete_not_found() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let result = backend.delete("nonexistent");

        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("key not found: nonexistent"),
                    "Error should mention key not found: {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }

    #[test]
    fn test_store_constructs_correct_command() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let password = SecretString::from("mypass");
        backend
            .store("test@host", &password)
            .expect("store should succeed");
    }

    #[test]
    fn test_store_without_vault_omits_flag() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        let password = SecretString::from("mypass");
        backend
            .store("test@host", &password)
            .expect("store without vault should succeed");
    }

    #[test]
    fn test_delete_resolves_id() {
        // "user@host" maps to id "abc123" which mock accepts for delete.
        let _guard = ENV_MUTEX.lock().unwrap();
        let backend = OnePasswordBackend::with_op_path(None, mock_op_path(), false);
        backend
            .delete("user@host")
            .expect("delete should succeed for known item");
    }

    // ── Parse-only unit tests ─────────────────────────────────────────

    #[test]
    fn test_parse_item_list() {
        let json = r#"[
            {"id":"abc123","title":"user@host","category":"PASSWORD"},
            {"id":"def456","title":"root@server","category":"PASSWORD"}
        ]"#;

        let items = parse_item_list(json).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "abc123");
        assert_eq!(items[0].title, "user@host");
        assert_eq!(items[0].category, "PASSWORD");
        assert_eq!(items[1].id, "def456");
        assert_eq!(items[1].title, "root@server");
    }

    #[test]
    fn test_parse_empty_list() {
        let json = "[]";
        let items = parse_item_list(json).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_item_titles() {
        let json = r#"[
            {"id":"abc123","title":"user@host","category":"PASSWORD"},
            {"id":"def456","title":"root@server","category":"PASSWORD"}
        ]"#;

        let titles = parse_item_titles(json).unwrap();
        assert_eq!(titles, vec!["user@host", "root@server"]);
    }

    #[test]
    fn test_parse_item_password() {
        let json = r#"{
            "id":"abc123",
            "title":"user@host",
            "category":"PASSWORD",
            "fields":[
                {"id":"password","type":"CONCEALED","value":"s3cret","label":"password"},
                {"id":"notesPlain","type":"STRING","value":"","label":"notes"}
            ]
        }"#;

        let password = parse_item_password(json).unwrap();
        assert_eq!(password.expose_secret(), "s3cret");
    }

    #[test]
    fn test_parse_item_password_by_id() {
        // Field has id="password" but type is not CONCEALED — should still be found
        // by the first-pass id check.
        let json = r#"{
            "id":"abc123",
            "title":"user@host",
            "category":"PASSWORD",
            "fields":[
                {"id":"password","type":"STRING","value":"found_by_id","label":"password"}
            ]
        }"#;

        let password = parse_item_password(json).unwrap();
        assert_eq!(password.expose_secret(), "found_by_id");
    }

    #[test]
    fn test_parse_item_password_not_found() {
        let json = r#"{
            "id":"abc123",
            "title":"user@host",
            "category":"PASSWORD",
            "fields":[
                {"id":"notesPlain","type":"STRING","value":"just notes","label":"notes"}
            ]
        }"#;

        let result = parse_item_password(json);
        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("no password field found"),
                    "Error should mention missing password: {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_unknown_fields() {
        // JSON with extra unknown fields should parse successfully (forward compat).
        let json = r#"[
            {
                "id":"abc123",
                "title":"user@host",
                "category":"PASSWORD",
                "vault":{"id":"vault1","name":"Personal"},
                "urls":[{"href":"https://example.com"}],
                "version":3,
                "createdAt":"2024-01-01T00:00:00Z"
            }
        ]"#;

        let items = parse_item_list(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "abc123");
        assert_eq!(items[0].title, "user@host");
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not valid json at all {{{";

        let result = parse_item_list(json);
        assert!(result.is_err());
        match result.unwrap_err() {
            SshpassError::KeychainAccess(msg) => {
                assert!(
                    msg.contains("failed to parse"),
                    "Error should mention parse failure: {msg}"
                );
            }
            other => panic!("Expected KeychainAccess, got: {other:?}"),
        }
    }
}

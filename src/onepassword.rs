use secrecy::SecretString;
use serde::Deserialize;

use crate::error::SshpassError;

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
    pub category: String,
}

/// Represents the full detail of a single item from `op item get --format json`.
///
/// Includes the `fields` array needed to extract password values.
#[derive(Debug, Deserialize)]
pub struct OpItemDetail {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

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

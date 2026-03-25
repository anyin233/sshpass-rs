use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Returns the absolute path to the mock `op` shell script.
///
/// Params: None.
/// Returns: String path to `tests/fixtures/mock_op.sh`.
fn mock_op_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/fixtures/mock_op.sh", manifest_dir)
}

/// Creates a temp directory with a symlink named `op` pointing to `mock_op.sh`,
/// then returns the temp dir (for lifetime) and a PATH string with the temp dir prepended.
///
/// Params: None.
/// Returns: (TempDir, String) — the temp dir guard and the augmented PATH.
fn setup_mock_op_in_path() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("expected tempdir for mock op");
    let mock_src = mock_op_path();
    let mock_dst = dir.path().join("op");
    std::os::unix::fs::symlink(&mock_src, &mock_dst)
        .expect("expected symlink creation for mock op");
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (dir, path)
}

/// Creates a temp keychain env for the file-based backend (existing behavior).
///
/// Params: None.
/// Returns: (TempDir, String) — the temp dir guard and the keychain file path.
fn temp_keychain_env() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("expected tempdir");
    let path = dir.path().join("keychain.json");
    (dir, path.display().to_string())
}

// GIVEN SSHPASSX_BACKEND=op with mock op on PATH,
// WHEN sshpass-rs --list,
// THEN exit 0 and stdout contains "user@host" and "root@server".
#[test]
fn test_list_with_op_backend() {
    let (_dir, path) = setup_mock_op_in_path();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("user@host"))
        .stdout(predicate::str::contains("root@server"));
}

// GIVEN SSHPASSX_BACKEND=op with mock op on PATH and MOCK_OP_EMPTY=1,
// WHEN sshpass-rs --list,
// THEN exit 0 and stdout contains "(empty)".
#[test]
fn test_list_empty_with_op_backend() {
    let (_dir, path) = setup_mock_op_in_path();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .env("MOCK_OP_EMPTY", "1")
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("(empty)"));
}

// GIVEN SSHPASSX_BACKEND=op with mock op on PATH and SSHPASSX_TEST_PASSWORD=testpass,
// WHEN sshpass-rs --store test@host,
// THEN exit 0 and stdout contains "Password stored".
#[test]
fn test_store_with_op_backend() {
    let (_dir, path) = setup_mock_op_in_path();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .env("SSHPASSX_TEST_PASSWORD", "testpass")
        .args(["--store", "test@host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password stored"));
}

// GIVEN SSHPASSX_BACKEND=op with mock op on PATH,
// WHEN sshpass-rs --delete user@host,
// THEN exit 0 and stdout contains "Password deleted".
#[test]
fn test_delete_with_op_backend() {
    let (_dir, path) = setup_mock_op_in_path();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .args(["--delete", "user@host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password deleted"));
}

// GIVEN SSHPASSX_BACKEND=op but op is NOT on PATH,
// WHEN sshpass-rs --list,
// THEN exit 3 (RuntimeError) and stderr contains "1Password CLI (op) not found".
#[test]
fn test_op_not_installed_error() {
    let empty_dir = tempfile::tempdir().expect("expected tempdir for empty PATH");
    let path = empty_dir.path().display().to_string();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .arg("--list")
        .assert()
        .code(3)
        .stderr(predicate::str::contains("1Password CLI (op) not found"));
}

// GIVEN no SSHPASSX_BACKEND set (default file backend),
// WHEN sshpass-rs --list with a fresh temp keychain file,
// THEN exit 0 and stdout contains "(empty)" — existing behavior preserved.
#[test]
fn test_existing_keychain_tests_unaffected() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", keychain_file)
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("(empty)"));
}

// GIVEN SSHPASSX_BACKEND=op with mock op on PATH and -v,
// WHEN sshpass-rs -v --list,
// THEN stderr contains "selected 1Password backend".
#[test]
fn test_verbose_op_backend_selection() {
    let (_dir, path) = setup_mock_op_in_path();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .args(["-v", "--list"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "SSHPASSX: selected 1Password backend",
        ));
}

// GIVEN SSHPASSX_BACKEND=op with mock op on PATH and -v,
// WHEN sshpass-rs -v --list,
// THEN stderr contains "running: op" showing the op command.
#[test]
fn test_verbose_op_command_shown() {
    let (_dir, path) = setup_mock_op_in_path();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "op")
        .env("PATH", &path)
        .args(["-v", "--list"])
        .assert()
        .success()
        .stderr(predicate::str::contains("SSHPASSX: running: op"));
}

// GIVEN SSHPASSX_BACKEND=invalid with -v and test keychain file,
// WHEN sshpass-rs -v --list,
// THEN stderr contains "unknown backend 'invalid'" warning.
#[test]
fn test_verbose_unknown_backend_warning() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_BACKEND", "invalid")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", keychain_file)
        .args(["-v", "--list"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "SSHPASSX: unknown backend 'invalid'",
        ));
}

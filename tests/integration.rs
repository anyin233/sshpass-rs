use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::TempDir;

fn fake_ssh_bin() -> String {
    assert_cmd::cargo::cargo_bin("fake_ssh")
        .to_str()
        .expect("fake_ssh binary path should be valid UTF-8")
        .to_string()
}

fn temp_keychain_env() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("expected tempdir");
    let path = dir.path().join("keychain.json");
    (dir, path.display().to_string())
}

// GIVEN -p testpass, WHEN fake_ssh --mode success, THEN exit 0 + stdout "Welcome"
#[test]
fn password_flag_success_exits_zero() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "testpass", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome"));
}

// GIVEN -p wrongpass, WHEN fake_ssh --mode wrong-password, THEN exit 5
#[test]
fn wrong_password_exits_five() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args([
            "-p",
            "wrongpass",
            &fake_ssh_bin(),
            "--mode",
            "wrong-password",
        ])
        .assert()
        .code(5);
}

// GIVEN -f <tempfile>, WHEN fake_ssh --mode success, THEN exit 0 + stdout "Welcome"
#[test]
fn file_flag_success_exits_zero() {
    let mut passfile = tempfile::NamedTempFile::new().expect("temp file");
    write!(passfile, "testpass\n").expect("write password");

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args([
            "-f",
            passfile.path().to_str().unwrap(),
            &fake_ssh_bin(),
            "--mode",
            "success",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome"));
}

// GIVEN -e + SSHPASS=testpass, WHEN fake_ssh --mode success, THEN exit 0 + stdout "Welcome"
#[test]
fn env_flag_success_exits_zero() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASS", "testpass")
        .args(["-e", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome"));
}

// GIVEN -p x -e (conflicting), WHEN any command, THEN exit 2 + "mutually exclusive"
#[test]
fn conflicting_sources_exits_two() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "x", "-e", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("mutually exclusive"));
}

// GIVEN -p pass, WHEN no command provided, THEN exit 1 + "missing wrapped command"
#[test]
fn missing_command_exits_one() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "pass"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("missing wrapped command"));
}

// GIVEN -p pass, WHEN fake_ssh --mode host-key-unknown, THEN exit 6
#[test]
fn host_key_unknown_exits_six() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "pass", &fake_ssh_bin(), "--mode", "host-key-unknown"])
        .assert()
        .code(6);
}

// GIVEN -p pass, WHEN fake_ssh --mode host-key-changed, THEN exit 7
#[test]
fn host_key_changed_exits_seven() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "pass", &fake_ssh_bin(), "--mode", "host-key-changed"])
        .assert()
        .code(7);
}

// GIVEN -P "custom:" -p testpass, WHEN fake_ssh --mode custom-prompt, THEN exit 0
#[test]
fn custom_prompt_exits_zero() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args([
            "-P",
            "custom:",
            "-p",
            "testpass",
            &fake_ssh_bin(),
            "--mode",
            "custom-prompt",
            "--prompt",
            "custom:",
        ])
        .assert()
        .success();
}

// GIVEN -v -p testpass, WHEN fake_ssh --mode success, THEN stderr contains "SSHPASS"
#[test]
fn verbose_flag_prints_sshpass_to_stderr() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-v", "-p", "testpass", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stderr(predicate::str::contains("SSHPASS"));
}

// GIVEN -p testpass, WHEN fake_ssh --mode split-prompt, THEN exit 0
#[test]
fn split_prompt_exits_zero() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "testpass", &fake_ssh_bin(), "--mode", "split-prompt"])
        .assert()
        .success();
}

// GIVEN --store user@host + test keychain env, THEN exit 0 + "Password stored"
#[test]
fn keychain_store_exits_zero() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("SSHPASSX_TEST_PASSWORD", "stored_pass")
        .args(["--store", "user@host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password stored"));
}

// GIVEN stored key, WHEN --list, THEN stdout contains "user@host"
#[test]
fn keychain_list_shows_stored_key() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("SSHPASSX_TEST_PASSWORD", "stored_pass")
        .args(["--store", "user@host"])
        .assert()
        .success();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .args(["--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("user@host"));
}

// GIVEN stored key, WHEN --delete user@host, THEN exit 0 + "Password deleted"
#[test]
fn keychain_delete_exits_zero() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("SSHPASSX_TEST_PASSWORD", "stored_pass")
        .args(["--store", "user@host"])
        .assert()
        .success();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .args(["--delete", "user@host"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password deleted"));
}

// GIVEN -p testpass, WHEN fake_ssh --mode exit-code --exit 42, THEN exit 42
#[test]
fn child_exit_code_passthrough() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args([
            "-p",
            "testpass",
            &fake_ssh_bin(),
            "--mode",
            "exit-code",
            "--exit",
            "42",
        ])
        .assert()
        .code(42);
}

// GIVEN -p pass, WHEN fake_ssh --mode parse-error (no prompt match + non-zero exit), THEN exit 4
#[test]
fn parse_error_exits_four() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "pass", &fake_ssh_bin(), "--mode", "parse-error"])
        .assert()
        .code(4);
}

// GIVEN stdin password via pipe (no flag), WHEN fake_ssh --mode success, THEN exit 0
#[test]
fn stdin_mode_success() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .write_stdin("testpass\n")
        .args([&fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome"));
}

// GIVEN fresh keychain, WHEN --list, THEN stdout contains "(empty)"
#[test]
fn keychain_list_empty_prints_empty() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .args(["--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("(empty)"));
}

// GIVEN fresh keychain, WHEN --delete nonexistent@host, THEN exit 3 + "key not found"
#[test]
fn keychain_delete_nonexistent_fails() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .args(["--delete", "nonexistent@host"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("key not found"));
}

// GIVEN -f /nonexistent/path, WHEN any command, THEN exit 3 + "password source error"
#[test]
fn file_flag_nonexistent_file_exits_three() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args([
            "-f",
            "/nonexistent/path/to/password.txt",
            &fake_ssh_bin(),
            "--mode",
            "success",
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("password source error"));
}

// GIVEN -e without SSHPASS env var, WHEN any command, THEN exit 3 + stderr mentions "SSHPASS"
#[test]
fn env_flag_without_sshpass_exits_three() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env_remove("SSHPASS")
        .args(["-e", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("SSHPASS"));
}

// GIVEN -p testpass, WHEN fake_ssh --mode no-prompt (exits 0 immediately), THEN exit 0
#[test]
fn no_prompt_child_exits_zero_passthrough() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-p", "testpass", &fake_ssh_bin(), "--mode", "no-prompt"])
        .assert()
        .success();
}

// GIVEN -d 0 (read password from fd 0/stdin via bash pipe), WHEN fake_ssh --mode success, THEN exit 0
#[test]
fn fd_flag_success_exits_zero() {
    let fake_ssh = fake_ssh_bin();
    let sshpass = assert_cmd::cargo::cargo_bin("sshpassx")
        .to_str()
        .unwrap()
        .to_string();

    Command::new("bash")
        .args([
            "-c",
            &format!("echo testpass | {sshpass} -d 0 {fake_ssh} --mode success"),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome"));
}

// GIVEN stored keychain password, WHEN --key user@host fake_ssh, THEN retrieves password + exit 0
#[test]
fn keychain_store_then_use_with_key_flag() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("SSHPASSX_TEST_PASSWORD", "kc_pass")
        .args(["--store", "user@host"])
        .assert()
        .success();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .args(["--key", "user@host", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Welcome"));
}

// GIVEN -v -p testpass, WHEN fake_ssh --mode success, THEN stderr has "detected" + "sending"
#[test]
fn verbose_shows_detected_and_sending() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-v", "-p", "testpass", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stderr(predicate::str::contains("SSHPASS detected password prompt"))
        .stderr(predicate::str::contains("SSHPASS sending password"));
}

// GIVEN -v -p testpass, WHEN fake_ssh --mode success, THEN stderr contains password source diagnostic
#[test]
fn test_verbose_password_source_p_flag() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args(["-v", "-p", "testpass", &fake_ssh_bin(), "--mode", "success"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "SSHPASSX: using password from -p argument",
        ));
}

// GIVEN -v --list with default backend, THEN stderr contains backend checking diagnostic
#[test]
fn test_verbose_backend_selection_default() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", keychain_file)
        .args(["-v", "--list"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "SSHPASSX: checking SSHPASSX_BACKEND",
        ));
}

// GIVEN -v --list, THEN stderr contains "listing stored keys" diagnostic
#[test]
fn test_verbose_standalone_list() {
    let (_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", keychain_file)
        .args(["-v", "--list"])
        .assert()
        .success()
        .stderr(predicate::str::contains("SSHPASSX: listing stored keys"));
}

fn setup_mock_ssh_in_path() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("expected tempdir for mock ssh");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mock_src = format!("{}/tests/fixtures/mock_ssh.sh", manifest_dir);
    let mock_dst = dir.path().join("ssh");
    std::os::unix::fs::symlink(&mock_src, &mock_dst)
        .expect("expected symlink creation for mock ssh");
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (dir, path)
}

// GIVEN mock_ssh in PATH + -v -k ssh myalias, THEN stderr shows alias resolution messages
#[test]
fn test_alias_resolution_verbose() {
    let (_ssh_dir, ssh_path) = setup_mock_ssh_in_path();
    let (_kc_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("PATH", &ssh_path)
        .args(["-v", "-k", "ssh", "myalias"])
        .assert()
        .stderr(predicate::str::contains(
            "SSHPASSX: resolving SSH alias 'myalias' via ssh -G",
        ))
        .stderr(predicate::str::contains(
            "SSHPASSX: resolved alias 'myalias' to keychain key 'testuser@10.0.0.1'",
        ));
}

// GIVEN mock_ssh in PATH + -v -k ssh -W %h:%p gw, THEN stderr shows gw resolved to admin@gateway.local
#[test]
fn test_w_flag_alias_resolution_verbose() {
    let (_ssh_dir, ssh_path) = setup_mock_ssh_in_path();
    let (_kc_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("PATH", &ssh_path)
        .args(["-v", "-k", "ssh", "-W", "%h:%p", "gw"])
        .assert()
        .stderr(predicate::str::contains(
            "SSHPASSX: resolved alias 'gw' to keychain key 'admin@gateway.local'",
        ));
}

// GIVEN --key explicit-key, WHEN ssh myalias, THEN uses explicit key and does NOT resolve alias
#[test]
fn test_key_override_bypasses_resolution() {
    let (_ssh_dir, ssh_path) = setup_mock_ssh_in_path();
    let (_kc_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("PATH", &ssh_path)
        .args(["-v", "--key", "explicit-key", "ssh", "myalias"])
        .assert()
        .stderr(predicate::str::contains(
            "SSHPASSX: using keychain with key 'explicit-key'",
        ))
        .stderr(predicate::str::contains("resolving SSH alias").not());
}

// GIVEN -v -k ssh user@host (direct user@host), THEN uses key directly without alias resolution
#[test]
fn test_direct_user_at_host_bypasses_resolution() {
    let (_ssh_dir, ssh_path) = setup_mock_ssh_in_path();
    let (_kc_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("PATH", &ssh_path)
        .args(["-v", "-k", "ssh", "user@host"])
        .assert()
        .stderr(predicate::str::contains(
            "SSHPASSX: using keychain with key 'user@host'",
        ))
        .stderr(predicate::str::contains("resolving SSH alias").not());
}

// GIVEN -v -k ssh -F /tmp/custom.cfg custom-alias, THEN resolves custom-alias to customuser@custom.example.com
#[test]
fn test_f_flag_propagation_in_resolution() {
    let (_ssh_dir, ssh_path) = setup_mock_ssh_in_path();
    let (_kc_dir, keychain_file) = temp_keychain_env();

    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .env("SSHPASSX_TEST_KEYCHAIN_FILE", &keychain_file)
        .env("PATH", &ssh_path)
        .args(["-v", "-k", "ssh", "-F", "/tmp/custom.cfg", "custom-alias"])
        .assert()
        .stderr(predicate::str::contains(
            "SSHPASSX: resolved alias 'custom-alias' to keychain key 'customuser@custom.example.com'",
        ));
}

// GIVEN -v -p SUPERSECRET_XYZ_123, WHEN fake_ssh --mode success, THEN secret NOT in stderr
#[test]
fn test_verbose_no_secret_leak() {
    Command::cargo_bin("sshpassx")
        .expect("binary exists")
        .args([
            "-v",
            "-p",
            "SUPERSECRET_XYZ_123",
            &fake_ssh_bin(),
            "--mode",
            "success",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("SUPERSECRET_XYZ_123").not());
}

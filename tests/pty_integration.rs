#[path = "../src/error.rs"]
mod error;
#[path = "../src/matcher.rs"]
mod matcher;
#[path = "../src/pty.rs"]
mod pty;
#[path = "../src/signals.rs"]
mod signals;

use matcher::PromptMatcher;
use pty::PtySession;
use secrecy::SecretString;
use std::io::Read;
use std::process::Command;
use std::time::{Duration, Instant};

fn read_until_timeout(reader: &mut dyn Read, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 256];

    while Instant::now() < deadline {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                output.extend_from_slice(&buffer[..count]);
                if output
                    .windows(b"Password:".len())
                    .any(|window| window == b"Password:")
                {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => panic!("failed reading PTY output: {err}"),
        }
    }

    String::from_utf8(output).expect("expected UTF-8 PTY output")
}

fn run_fake_ssh(
    mode: &str,
    extra_args: &[&str],
    prompt: &str,
    password: &str,
    verbose: bool,
) -> i32 {
    let fake_ssh = env!("CARGO_BIN_EXE_fake_ssh").to_string();
    let mut command = vec![fake_ssh, "--mode".to_string(), mode.to_string()];

    for arg in extra_args {
        command.push((*arg).to_string());
    }

    let mut session = PtySession::new().expect("expected PTY creation to succeed");
    session
        .spawn_command(&command)
        .expect("expected fake_ssh to spawn");
    let secret = SecretString::from(password.to_string());
    let mut matcher = PromptMatcher::new(prompt);

    let exit_code = session
        .run_with_password(&secret, &mut matcher, None, verbose)
        .expect("expected run_with_password to succeed");

    exit_code
}

#[test]
fn test_spawn_and_read() {
    let fake_ssh = env!("CARGO_BIN_EXE_fake_ssh").to_string();
    let mut session = PtySession::new().expect("expected PTY creation to succeed");
    let command = vec![fake_ssh, "--mode".to_string(), "success".to_string()];
    session
        .spawn_command(&command)
        .expect("expected fake_ssh to spawn");
    let mut reader = session.take_reader().expect("expected PTY reader");

    let output = read_until_timeout(reader.as_mut(), Duration::from_secs(2));

    assert!(
        output.contains("Password:"),
        "expected password prompt in PTY output, got: {output:?}"
    );
}

#[test]
fn test_spawn_fake_ssh_no_prompt() {
    let fake_ssh = env!("CARGO_BIN_EXE_fake_ssh").to_string();
    let mut session = PtySession::new().expect("expected PTY creation to succeed");
    let command = vec![fake_ssh, "--mode".to_string(), "no-prompt".to_string()];
    session
        .spawn_command(&command)
        .expect("expected fake_ssh to spawn");

    let exit_code = session
        .wait_for_child()
        .expect("expected child exit status");

    assert_eq!(exit_code, 0, "expected fake_ssh to exit successfully");
}

#[test]
fn test_successful_auth() {
    let exit_code = run_fake_ssh("success", &[], "assword:", "correct-password", false);

    assert_eq!(exit_code, 0, "expected successful auth exit code");
}

#[test]
fn test_wrong_password() {
    let exit_code = run_fake_ssh("wrong-password", &[], "assword:", "wrong-password", false);

    assert_eq!(exit_code, 5, "expected wrong password exit code");
}

#[test]
fn test_split_prompt() {
    let exit_code = run_fake_ssh("split-prompt", &[], "assword:", "correct-password", false);

    assert_eq!(exit_code, 0, "expected split prompt auth exit code");
}

#[test]
fn test_custom_prompt() {
    let exit_code = run_fake_ssh(
        "custom-prompt",
        &["--prompt", "Secret: "],
        "Secret: ",
        "correct-password",
        false,
    );

    assert_eq!(exit_code, 0, "expected custom prompt auth exit code");
}

#[test]
fn test_host_key_unknown() {
    let exit_code = run_fake_ssh(
        "host-key-unknown",
        &[],
        "assword:",
        "correct-password",
        false,
    );

    assert_eq!(exit_code, 6, "expected host key unknown exit code");
}

#[test]
fn test_host_key_changed() {
    let exit_code = run_fake_ssh(
        "host-key-changed",
        &[],
        "assword:",
        "correct-password",
        false,
    );

    assert_eq!(exit_code, 7, "expected host key changed exit code");
}

#[test]
fn test_exit_code_forward() {
    let exit_code = run_fake_ssh(
        "exit-code",
        &["--exit", "42"],
        "assword:",
        "correct-password",
        false,
    );

    assert_eq!(exit_code, 42, "expected child exit code to be forwarded");
}

#[test]
fn test_parse_error() {
    let exit_code = run_fake_ssh("parse-error", &[], "assword:", "correct-password", false);

    assert_eq!(exit_code, 4, "expected parse error exit code");
}

#[test]
fn test_verbose_mode() {
    let output = Command::new(std::env::current_exe().expect("expected current test binary path"))
        .args(["--exact", "verbose_capture_helper", "--nocapture"])
        .env("SSHPASS_RS_VERBOSE_HELPER", "1")
        .output()
        .expect("expected verbose helper subprocess to run");
    let stderr = String::from_utf8(output.stderr).expect("expected UTF-8 stderr");

    assert!(
        output.status.success(),
        "expected verbose helper subprocess to succeed, stderr: {stderr:?}"
    );
    assert!(
        stderr.contains("SSHPASS searching for password prompt using match \"assword:\""),
        "expected verbose search output, got: {stderr:?}"
    );
    assert!(
        stderr.contains("SSHPASS detected password prompt"),
        "expected verbose prompt detection output, got: {stderr:?}"
    );
    assert!(
        stderr.contains("SSHPASS sending password"),
        "expected verbose password send output, got: {stderr:?}"
    );
}

#[test]
fn test_no_prompt_exit() {
    let exit_code = run_fake_ssh("no-prompt", &[], "assword:", "correct-password", false);

    assert_eq!(exit_code, 0, "expected no-prompt child exit code");
}

#[test]
fn verbose_capture_helper() {
    if std::env::var_os("SSHPASS_RS_VERBOSE_HELPER").is_none() {
        return;
    }

    let exit_code = run_fake_ssh("success", &[], "assword:", "correct-password", true);

    assert_eq!(exit_code, 0, "expected successful auth exit code");
}

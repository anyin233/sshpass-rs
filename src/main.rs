mod cli;
mod error;
mod keychain;
mod matcher;
mod onepassword;
mod password;
mod pty;
mod signals;

use crate::cli::Cli;
use crate::error::{SshpassError, SshpassExitCode};
use crate::keychain::{FileKeychainBackend, KeychainBackend, KeychainManager, RealKeychainBackend};
use crate::matcher::PromptMatcher;
use crate::onepassword::OnePasswordBackend;
use crate::password::PasswordResolver;
use crate::pty::PtySession;

fn main() {
    std::process::exit(run());
}

/// Runs the complete sshpass-rs CLI orchestration and returns the process exit code.
///
/// Params:
/// - None.
///
/// Returns:
/// - The exit code for the current invocation.
fn run() -> i32 {
    let cli = match Cli::parse() {
        Ok(cli) => cli,
        Err((message, exit_code)) => {
            eprintln!("{message}");
            return exit_code;
        }
    };

    let keychain_manager = build_keychain_manager(cli.verbose);
    if let Some(exit_code) = handle_standalone(&cli, &keychain_manager, cli.verbose) {
        return exit_code;
    }

    let password = match resolve_password(&cli, cli.verbose) {
        Ok(password) => password,
        Err(err) => return report_runtime_error(err),
    };

    let mut pty_session = match PtySession::new() {
        Ok(session) => session,
        Err(err) => return report_runtime_error(err),
    };

    let command = match normalize_command(&cli.command) {
        Ok(command) => command,
        Err(err) => return report_runtime_error(err),
    };

    if let Err(err) = pty_session.spawn_command(&command) {
        return report_runtime_error(err);
    }

    let signal_handler = match build_signal_handler(&mut pty_session) {
        Ok(handler) => handler,
        Err(err) => return report_runtime_error(err),
    };

    let mut matcher = PromptMatcher::new(&cli.prompt);
    match pty_session.run_with_password(&password, &mut matcher, Some(&signal_handler), cli.verbose)
    {
        Ok(exit_code) => exit_code,
        Err(err) => report_runtime_error(err),
    }
}

/// Creates the runtime keychain manager chosen by the current environment.
///
/// Params:
/// - None.
///
/// Returns:
/// - A keychain manager backed by either the file backend or the real OS keychain.
fn build_keychain_manager(verbose: bool) -> KeychainManager {
    KeychainManager::new(build_keychain_backend(verbose))
}

/// Creates a fresh keychain backend matching the active environment configuration.
///
/// Params:
/// - None.
///
/// Returns:
/// - A boxed keychain backend for standalone operations or password resolution.
fn build_keychain_backend(verbose: bool) -> Box<dyn KeychainBackend> {
    if verbose {
        eprintln!("SSHPASS_RS: checking SSHPASS_RS_BACKEND environment variable");
    }

    if let Ok(backend) = std::env::var("SSHPASS_RS_BACKEND") {
        if backend == "op" || backend == "1password" {
            let vault = std::env::var("SSHPASS_RS_VAULT").ok();
            if verbose {
                eprintln!(
                    "SSHPASS_RS: selected 1Password backend (vault: {})",
                    vault.as_deref().unwrap_or("default")
                );
            }
            return Box::new(OnePasswordBackend::new(vault, verbose));
        }

        if verbose {
            eprintln!(
                "SSHPASS_RS: unknown backend '{}', falling back to OS keychain",
                backend
            );
        }
    }

    match std::env::var("SSHPASS_RS_TEST_KEYCHAIN_FILE") {
        Ok(path) => Box::new(FileKeychainBackend::new(path)),
        Err(_) => {
            if verbose {
                eprintln!("SSHPASS_RS: selected OS keychain backend");
            }
            Box::new(RealKeychainBackend::new(verbose))
        }
    }
}

/// Runs standalone keychain operations before any PTY or password-handshake work starts.
///
/// Params:
/// - cli: Parsed command-line state.
/// - manager: Keychain manager used by standalone operations.
///
/// Returns:
/// - `Some(exit_code)` when a standalone operation ran, otherwise `None`.
fn handle_standalone(cli: &Cli, manager: &KeychainManager, verbose: bool) -> Option<i32> {
    let result = if let Some(key) = &cli.store {
        if verbose {
            eprintln!("SSHPASS_RS: performing store for key '{}'", key);
        }
        Some(keychain::handle_store(manager, key))
    } else if let Some(key) = &cli.delete {
        if verbose {
            eprintln!("SSHPASS_RS: performing delete for key '{}'", key);
        }
        Some(keychain::handle_delete(manager, key))
    } else if cli.list {
        if verbose {
            eprintln!("SSHPASS_RS: listing stored keys");
        }
        Some(keychain::handle_list(manager))
    } else {
        None
    };

    result.map(|operation| match operation {
        Ok(()) => SshpassExitCode::Success.into(),
        Err(err) => report_runtime_error(err),
    })
}

/// Resolves the password before PTY creation so interactive keychain prompting happens early.
///
/// Params:
/// - cli: Parsed command-line state.
///
/// Returns:
/// - The resolved password secret, or a runtime error.
fn resolve_password(cli: &Cli, verbose: bool) -> Result<secrecy::SecretString, SshpassError> {
    let resolver = if let Some(password) = &cli.password {
        if verbose {
            eprintln!("SSHPASS_RS: using password from -p argument");
        }
        PasswordResolver::Argument(password.clone())
    } else if let Some(filename) = &cli.filename {
        if verbose {
            eprintln!("SSHPASS_RS: using password from file '{}'", filename);
        }
        PasswordResolver::File(filename.into())
    } else if let Some(fd) = cli.fd {
        if verbose {
            eprintln!("SSHPASS_RS: using password from file descriptor {}", fd);
        }
        PasswordResolver::FileDescriptor(fd)
    } else if cli.use_env {
        if verbose {
            eprintln!("SSHPASS_RS: using password from SSHPASS environment variable");
        }
        PasswordResolver::Environment
    } else if let Some(key) = &cli.key {
        if verbose {
            eprintln!("SSHPASS_RS: using keychain with key '{}'", key);
        }
        PasswordResolver::Keychain(key.clone())
    } else if cli.use_keychain {
        let key = cli::parse_user_at_host(&cli.command).ok_or_else(|| {
            SshpassError::PasswordSource(
                "unable to derive keychain key from wrapped SSH arguments".to_string(),
            )
        })?;
        if verbose {
            eprintln!("SSHPASS_RS: using keychain with key '{}'", key);
        }
        PasswordResolver::Keychain(key)
    } else {
        if verbose {
            eprintln!("SSHPASS_RS: reading password from stdin");
        }
        PasswordResolver::Stdin
    };

    match resolver {
        PasswordResolver::Keychain(_) => {
            resolver.resolve_with_keychain(build_keychain_backend(verbose), verbose)
        }
        _ => resolver.resolve(),
    }
}

/// Rewrites explicit filesystem command paths to absolute paths before PTY spawning.
///
/// Params:
/// - command: The wrapped command plus arguments.
///
/// Returns:
/// - A spawnable command vector with an absolute program path when one was provided.
fn normalize_command(command: &[String]) -> Result<Vec<String>, SshpassError> {
    let Some(program) = command.first() else {
        return Ok(Vec::new());
    };

    let mut normalized = command.to_vec();
    let program_path = std::path::Path::new(program);
    if program_path.components().count() > 1 && program_path.is_relative() {
        let absolute = std::env::current_dir()?.join(program_path);
        normalized[0] = absolute.display().to_string();
    }

    Ok(normalized)
}

/// Builds and registers signal forwarding for the active PTY child process.
///
/// Params:
/// - session: The spawned PTY session whose master fd and child pid are needed.
///
/// Returns:
/// - A registered signal handler ready for the PTY I/O loop.
fn build_signal_handler(session: &mut PtySession) -> Result<signals::SignalHandler, SshpassError> {
    let child_pid = session
        .child_process_id()
        .ok_or_else(|| SshpassError::ChildSpawn("PTY child pid is unavailable".to_string()))?;
    let child_pid = i32::try_from(child_pid)
        .map_err(|_| SshpassError::ChildSpawn("PTY child pid exceeds i32 range".to_string()))?;
    let handler = signals::SignalHandler::new(session.master_fd()?, child_pid);
    handler.register_all().map_err(SshpassError::Io)?;
    Ok(handler)
}

/// Prints a runtime error and returns its corresponding sshpass exit code.
///
/// Params:
/// - err: The runtime error to surface to the user.
///
/// Returns:
/// - The mapped runtime exit code.
fn report_runtime_error(err: SshpassError) -> i32 {
    eprintln!("{err}");
    SshpassExitCode::from(&err).into()
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::TempDir;

    fn temp_keychain_env() -> (TempDir, String) {
        let dir = tempfile::tempdir().expect("expected tempdir");
        let path = dir.path().join("keychain.json");
        (dir, path.display().to_string())
    }

    #[test]
    fn list_standalone_prints_empty_for_fresh_store() {
        let (_dir, keychain_file) = temp_keychain_env();

        Command::cargo_bin("sshpass-rs")
            .expect("expected sshpass-rs binary")
            .env("SSHPASS_RS_TEST_KEYCHAIN_FILE", keychain_file)
            .arg("--list")
            .assert()
            .success()
            .stdout(predicate::str::contains("(empty)"));
    }

    #[test]
    fn verbose_list_standalone_logs_backend_selection_and_operation() {
        let (_dir, keychain_file) = temp_keychain_env();

        Command::cargo_bin("sshpass-rs")
            .expect("expected sshpass-rs binary")
            .env("SSHPASS_RS_TEST_KEYCHAIN_FILE", keychain_file)
            .args(["-v", "--list"])
            .assert()
            .success()
            .stderr(predicate::str::contains(
                "SSHPASS_RS: checking SSHPASS_RS_BACKEND environment variable",
            ))
            .stderr(predicate::str::contains("SSHPASS_RS: listing stored keys"));
    }

    #[test]
    fn conflicting_password_sources_exit_with_code_two() {
        Command::cargo_bin("sshpass-rs")
            .expect("expected sshpass-rs binary")
            .args([
                "-p",
                "x",
                "-e",
                "target/debug/fake_ssh",
                "--mode",
                "success",
            ])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("mutually exclusive"));
    }

    #[test]
    fn successful_fake_ssh_flow_exits_zero() {
        Command::cargo_bin("sshpass-rs")
            .expect("expected sshpass-rs binary")
            .args([
                "-p",
                "testpass",
                "target/debug/fake_ssh",
                "--mode",
                "success",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Welcome!"));
    }

    #[test]
    fn wrong_password_flow_exits_five() {
        Command::cargo_bin("sshpass-rs")
            .expect("expected sshpass-rs binary")
            .args([
                "-p",
                "wrongpass",
                "target/debug/fake_ssh",
                "--mode",
                "wrong-password",
            ])
            .assert()
            .code(5);
    }
}

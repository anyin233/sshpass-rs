use clap::Parser;

#[derive(Debug, Clone, Parser, PartialEq, Eq)]
#[command(disable_help_flag = true, disable_version_flag = true)]
pub struct Cli {
    #[arg(short = 'p')]
    pub password: Option<String>,

    #[arg(short = 'f')]
    pub filename: Option<String>,

    #[arg(short = 'd')]
    pub fd: Option<i32>,

    #[arg(short = 'e')]
    pub use_env: bool,

    #[arg(short = 'P', default_value = "assword:")]
    pub prompt: String,

    #[arg(short = 'v')]
    pub verbose: bool,

    #[arg(short = 'k')]
    pub use_keychain: bool,

    #[arg(long = "key")]
    pub key: Option<String>,

    #[arg(long = "store")]
    pub store: Option<String>,

    #[arg(long = "delete")]
    pub delete: Option<String>,

    #[arg(long = "list")]
    pub list: bool,

    #[arg(long = "help", short = 'h')]
    pub help: bool,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

impl Cli {
    /// Parses CLI arguments from the current process.
    ///
    /// Params:
    /// - None.
    ///
    /// Returns:
    /// - Parsed and validated CLI state, or a message plus process exit code.
    pub fn parse() -> Result<Self, (String, i32)> {
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        Self::parse_from(args)
    }

    /// Parses CLI arguments from a provided argv slice.
    ///
    /// Params:
    /// - args: Command-line arguments excluding the binary name.
    ///
    /// Returns:
    /// - Parsed and validated CLI state, or a message plus process exit code.
    pub fn parse_from(args: Vec<String>) -> Result<Self, (String, i32)> {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push("sshpassx".to_string());
        argv.extend(args);

        let cli = Self::try_parse_from(argv).map_err(|error| {
            let code = match error.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 2,
            };

            (error.to_string(), code)
        })?;

        cli.validate()?;
        Ok(cli)
    }

    /// Returns whether the current invocation is a standalone Keychain operation.
    ///
    /// Params:
    /// - None.
    ///
    /// Returns:
    /// - `true` when no wrapped SSH command is required.
    pub fn is_standalone(&self) -> bool {
        self.store.is_some() || self.delete.is_some() || self.list
    }

    /// Prints context-sensitive help based on which flags are present.
    ///
    /// Params:
    /// - None.
    ///
    /// Returns:
    /// - Nothing. Writes help text to stdout.
    pub fn print_help(&self) {
        if self.store.is_some() {
            Self::print_store_help();
        } else if self.delete.is_some() {
            Self::print_delete_help();
        } else if self.list {
            Self::print_list_help();
        } else if self.use_keychain || self.key.is_some() {
            Self::print_keychain_help();
        } else {
            Self::print_general_help();
        }
    }

    fn print_general_help() {
        println!(
            "\
sshpassx — non-interactive SSH password provider

USAGE:
    sshpassx [OPTIONS] <command> [args...]
    sshpassx --store <key>
    sshpassx --delete <key>
    sshpassx --list

PASSWORD SOURCE FLAGS (mutually exclusive):
    -p <password>    Pass the password directly as an argument
    -f <filename>    Read the password from a file (first line)
    -d <number>      Read the password from a file descriptor
    -e               Read the password from the SSHPASS environment variable
    -k               Look up the password from the configured backend,
                     auto-deriving the key from the wrapped SSH command

OTHER FLAGS:
    -P <prompt>      Prompt pattern to match (default: \"assword:\")
    -v               Verbose mode; prints diagnostic output to stderr
    --key <value>    Explicit key name to use with -k (overrides auto-detection)
    -h, --help       Show this help message

KEYCHAIN MANAGEMENT (standalone, no wrapped command needed):
    --store <key>    Prompt for a password and store it under <key>
    --delete <key>   Delete the stored entry for <key>
    --list           List all entries managed by sshpassx

BACKEND SELECTION (environment variables):
    SSHPASSX_BACKEND     Set to \"op\" or \"1password\" for 1Password backend.
                         Default: macOS Keychain.
                         (Legacy SSHPASS_RS_BACKEND is also accepted.)
    SSHPASSX_VAULT       1Password vault to use (optional, default vault if unset).
                         (Legacy SSHPASS_RS_VAULT is also accepted.)

EXAMPLES:
    sshpassx -p mypass ssh user@host
    sshpassx -k ssh user@host
    sshpassx --store user@host
    sshpassx --list
    SSHPASSX_BACKEND=op sshpassx -k ssh user@host

Use --store --help, --list --help, or -k --help for command-specific help."
        );
    }

    fn print_store_help() {
        println!(
            "\
sshpassx --store — store a password in the configured backend

USAGE:
    sshpassx --store <key>

DESCRIPTION:
    Prompts for a password interactively and stores it under <key>.
    The key is typically in the format \"user@host\".

    The storage backend is determined by the SSHPASSX_BACKEND environment
    variable. Default: macOS Keychain. Set to \"op\" for 1Password.
    (Legacy SSHPASS_RS_BACKEND is also accepted.)

OPTIONS:
    -v               Verbose mode; shows which backend is used
    -h, --help       Show this help message

ENVIRONMENT:
    SSHPASSX_BACKEND     \"op\" or \"1password\" → 1Password; unset → macOS Keychain
    SSHPASSX_VAULT       1Password vault name (optional)

EXAMPLES:
    sshpassx --store user@host
    SSHPASSX_BACKEND=op sshpassx --store user@host"
        );
    }

    fn print_delete_help() {
        println!(
            "\
sshpassx --delete — remove a stored password

USAGE:
    sshpassx --delete <key>

DESCRIPTION:
    Deletes the password stored under <key> from the configured backend.

OPTIONS:
    -v               Verbose mode; shows which backend is used
    -h, --help       Show this help message

ENVIRONMENT:
    SSHPASSX_BACKEND     \"op\" or \"1password\" → 1Password; unset → macOS Keychain
    SSHPASSX_VAULT       1Password vault name (optional)

EXAMPLES:
    sshpassx --delete user@host
    SSHPASSX_BACKEND=op sshpassx --delete user@host"
        );
    }

    fn print_list_help() {
        println!(
            "\
sshpassx --list — list stored passwords

USAGE:
    sshpassx --list

DESCRIPTION:
    Lists all password entries managed by sshpassx in the configured backend.
    Only entries tagged/indexed by sshpassx are shown.

OPTIONS:
    -v               Verbose mode; shows which backend is used
    -h, --help       Show this help message

ENVIRONMENT:
    SSHPASSX_BACKEND     \"op\" or \"1password\" → 1Password; unset → macOS Keychain
    SSHPASSX_VAULT       1Password vault name (optional)

EXAMPLES:
    sshpassx --list
    SSHPASSX_BACKEND=op sshpassx --list"
        );
    }

    fn print_keychain_help() {
        println!(
            "\
sshpassx -k — use stored password for SSH authentication

USAGE:
    sshpassx -k <command> [args...]
    sshpassx -k --key <name> <command> [args...]

DESCRIPTION:
    Looks up the password from the configured backend and uses it to
    authenticate the wrapped SSH command. The key is auto-derived from
    the SSH arguments (user@host) unless --key is specified.

    If the key is not found, falls back to an interactive password prompt
    and offers to save the password for future use.

OPTIONS:
    --key <value>    Use an explicit key name instead of auto-deriving
    -P <prompt>      Prompt pattern to match (default: \"assword:\")
    -v               Verbose mode; shows backend queries and results
    -h, --help       Show this help message

ENVIRONMENT:
    SSHPASSX_BACKEND     \"op\" or \"1password\" → 1Password; unset → macOS Keychain
    SSHPASSX_VAULT       1Password vault name (optional)

EXAMPLES:
    sshpassx -k ssh user@host
    sshpassx -k --key myserver ssh root@10.0.0.1
    SSHPASSX_BACKEND=op sshpassx -k ssh user@host"
        );
    }

    /// Validates sshpass compatibility rules after clap parsing.
    ///
    /// Params:
    /// - None.
    ///
    /// Returns:
    /// - `Ok(())` when the CLI is valid, otherwise a message plus process exit code.
    fn validate(&self) -> Result<(), (String, i32)> {
        let password_source_count = [
            self.password.is_some(),
            self.filename.is_some(),
            self.fd.is_some(),
            self.use_env,
            self.use_keychain,
        ]
        .into_iter()
        .filter(|is_set| *is_set)
        .count();

        if password_source_count > 1 {
            return Err((
                "password sources -p, -f, -d, -e, and -k are mutually exclusive".to_string(),
                2,
            ));
        }

        if !self.is_standalone() && !self.help && self.command.is_empty() {
            return Err(("missing wrapped command".to_string(), 1));
        }

        Ok(())
    }
}

/// Derives a `user@host` key name from supported SSH argument patterns.
///
/// Params:
/// - args: Wrapped SSH command and trailing arguments.
///
/// Returns:
/// - A derived `user@host` string for supported patterns, otherwise `None`.
pub fn parse_user_at_host(args: &[String]) -> Option<String> {
    let mut user = None;
    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];

        if index == 1 && !arg.starts_with('-') && arg.contains('@') {
            return Some(arg.clone());
        }

        if arg == "-l" {
            user = args.get(index + 1).cloned();
            index += 2;
            continue;
        }

        if !arg.starts_with('-') {
            return user.map(|username| format!("{username}@{arg}"));
        }

        index += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{parse_user_at_host, Cli};

    fn parse_ok(args: &[&str]) -> Cli {
        let owned = args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>();
        Cli::parse_from(owned).expect("expected parse success")
    }

    fn parse_err(args: &[&str]) -> (String, i32) {
        let owned = args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>();
        Cli::parse_from(owned).expect_err("expected parse failure")
    }

    #[test]
    fn test_basic_p_flag() {
        let cli = parse_ok(&["-p", "pass", "ssh", "user@host"]);

        assert_eq!(cli.password.as_deref(), Some("pass"));
        assert_eq!(cli.command, vec!["ssh", "user@host"]);
    }

    #[test]
    fn test_f_flag() {
        let cli = parse_ok(&["-f", "/tmp/pass", "ssh", "host"]);

        assert_eq!(cli.filename.as_deref(), Some("/tmp/pass"));
    }

    #[test]
    fn test_e_flag() {
        let cli = parse_ok(&["-e", "ssh", "host"]);

        assert!(cli.use_env);
    }

    #[test]
    fn test_k_flag_auto_derive() {
        let cli = parse_ok(&["-k", "ssh", "user@host"]);

        assert!(cli.use_keychain);
        assert_eq!(cli.key.as_deref(), None);
    }

    #[test]
    fn test_key_flag_explicit() {
        let cli = parse_ok(&["--key", "mykey", "ssh", "host"]);

        assert_eq!(cli.key.as_deref(), Some("mykey"));
    }

    #[test]
    fn test_conflicting_sources() {
        let (_, exit_code) = parse_err(&["-p", "x", "-e", "ssh"]);

        assert_eq!(exit_code, 2);
    }

    #[test]
    fn test_store_standalone() {
        let cli = parse_ok(&["--store", "user@host"]);

        assert_eq!(cli.store.as_deref(), Some("user@host"));
        assert!(cli.command.is_empty());
        assert!(cli.is_standalone());
    }

    #[test]
    fn test_list_standalone() {
        let cli = parse_ok(&["--list"]);

        assert!(cli.list);
        assert!(cli.command.is_empty());
        assert!(cli.is_standalone());
    }

    #[test]
    fn test_delete_standalone() {
        let cli = parse_ok(&["--delete", "user@host"]);

        assert_eq!(cli.delete.as_deref(), Some("user@host"));
        assert!(cli.command.is_empty());
        assert!(cli.is_standalone());
    }

    #[test]
    fn test_no_flag_stdin() {
        let cli = parse_ok(&["ssh", "user@host"]);

        assert_eq!(cli.password, None);
        assert_eq!(cli.filename, None);
        assert_eq!(cli.fd, None);
        assert!(!cli.use_env);
        assert!(!cli.use_keychain);
        assert_eq!(cli.command, vec!["ssh", "user@host"]);
    }

    #[test]
    fn test_ssh_passthrough() {
        let cli = parse_ok(&[
            "-p",
            "x",
            "ssh",
            "-v",
            "-o",
            "StrictHostKeyChecking=no",
            "user@host",
        ]);

        assert_eq!(
            cli.command,
            vec!["ssh", "-v", "-o", "StrictHostKeyChecking=no", "user@host"]
        );
    }

    #[test]
    fn test_user_at_host_direct() {
        let args = vec!["ssh".to_string(), "user@host".to_string()];

        assert_eq!(parse_user_at_host(&args).as_deref(), Some("user@host"));
    }

    #[test]
    fn test_user_at_host_l_flag() {
        let args = vec![
            "ssh".to_string(),
            "-l".to_string(),
            "user".to_string(),
            "host".to_string(),
        ];

        assert_eq!(parse_user_at_host(&args).as_deref(), Some("user@host"));
    }

    #[test]
    fn test_missing_command() {
        let (_, exit_code) = parse_err(&["-p", "pass"]);

        assert_eq!(exit_code, 1);
    }
}

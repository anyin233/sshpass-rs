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
        argv.push("sshpass-rs".to_string());
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

        if !self.is_standalone() && self.command.is_empty() {
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

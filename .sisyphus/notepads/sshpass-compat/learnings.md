# Learnings

## 2026-03-28 Session Start
- Project uses `clap` for CLI parsing with `disable_help_flag = true, disable_version_flag = true`
- Tests use `assert_cmd` + `predicates` + `fake_ssh` binary
- Exit codes: 0-7 matching original sshpass enum
- PTY uses `portable-pty` crate + raw libc for child spawn
- Signal handling via `signal-hook` crate with AtomicBool flags

## T2: No-arguments shows help + exit 0

- Removed the `!is_standalone() && !help && command.is_empty()` error block from `cli.rs validate()` — validation now passes with empty command
- Added empty-command check in `main.rs run()` after standalone handling: `if cli.command.is_empty() { cli.print_help(); return 0; }`
- The `is_standalone()` method now has a dead_code warning since validate() no longer calls it — this is expected, it's still used in main.rs via `handle_standalone`
- Unit test `test_missing_command` updated: now asserts `parse_ok` succeeds and `command.is_empty()` rather than expecting a parse error
- Integration test renamed `missing_command_exits_one` → `missing_command_shows_help_exits_zero`, asserts exit 0 + stdout contains "sshpassx"
- New integration test `no_arguments_shows_help_exits_zero` added
- Pre-existing failure: `list_standalone_prints_empty_for_fresh_store` fails due to real keychain having data — unrelated to this task

## T10: PTY-close child termination

- `run_with_password()` now mirrors original sshpass behavior on wrong-password / host-key error paths by dropping PTY reader/writer handles and the slave before waiting on the child
- Closing the PTY causes the child to observe controlling-terminal hangup semantics instead of receiving an immediate `SIGKILL`
- The normal success and EOF cleanup paths already used the same descriptor-drop sequence, so the compat fix kept the existing flow and reused that cleanup pattern
- Local verification required unsetting `SSHPASSX_BACKEND`/`SSHPASS_RS_BACKEND`; otherwise standalone list tests were diverted to the operator's configured 1Password backend instead of the file-backed test backend

## T11: Deferred password resolution

- Added a cached `Password` wrapper so stdin/file/fd sources can stay unresolved until `run_with_password_source()` sees the password prompt, while `-p`, `-e`, and keychain-backed flows still materialize a `SecretString` up front
- Legacy-stdio PTY sessions must keep stdin forwarding disabled while a deferred password is still pending; otherwise stdin/`-d 0` data is forwarded to the child before prompt handling and the later deferred read sees EOF/EIO instead of the password
- `env -u SSHPASSX_BACKEND -u SSHPASS_RS_BACKEND cargo test` is the reliable full-suite verification command for this compat wave because it avoids unrelated backend selection leakage from the shell environment

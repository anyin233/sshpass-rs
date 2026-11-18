# Decisions

## 2026-03-28 Plan Decisions
- Keep HOST_KEY_CHANGED detection (exit 7) as sshpassx enhancement
- Keep SSHPASS env var clearing as security improvement
- Skip argv obfuscation for -p (too complex in Rust, low value)
- Defer stdin/fd password reading to match original lazy behavior

## T10: PTY-close termination

- Keep the existing `run_with_password()` control flow and replace only the three error termination branches with the same PTY-drop sequence already used on normal shutdown
- Remove `terminate_child()` because normal compatibility behavior should be PTY close + `wait()`, not forced `SIGKILL`

## T11: Deferred password resolution

- Keep the public `run_with_password(&SecretString, ...)` helper for existing PTY tests and add `run_with_password_source(&mut Password, ...)` for the deferred compat path used by `main.rs`
- Gate legacy stdin forwarding on whether the password is still deferred so prompt-time password reads from stdin / `-d 0` happen before any interactive passthrough

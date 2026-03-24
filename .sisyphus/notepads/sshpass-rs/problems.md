
## Compliance audit blockers (2026-03-24)
- Missing sacred plan file: `.sisyphus/plans/sshpass-rs.md`.
- `src/cli.rs` disables help/version flags, so full sshpass CLI compatibility is not met.
- `src/password.rs` and `src/keychain.rs` keep password data in bare `String` values.
- `src/pty.rs` treats `poll()` EINTR as a hard I/O error, which weakens signal-forwarding correctness.
- Multiple modules exceed the 300-line maximum.

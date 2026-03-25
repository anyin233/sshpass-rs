# Learnings — 1Password Backend

## 2026-03-24 Session Start
- Base commit: 29d986f0b52f2ce9850a517450bdcadd22e3afc4
- Working directory: /Users/cydia2001/Project/sshpass-rs
- Plan: .sisyphus/plans/1password-backend.md

- Task 4: Implemented OnePasswordBackend in src/onepassword.rs with list+exact-title filtering for get/delete, stdin JSON for store, and helpful op-not-found subprocess error mapping.

## 2026-03-24 Scope Fidelity Check
- Commit scopes from 29d986f..HEAD matched the declared file strategy with no extra files per commit.
- The protected files src/keychain.rs, src/pty.rs, src/matcher.rs, src/signals.rs, and src/cli.rs had zero diff versus the base commit.
- src/onepassword.rs contained no matches for fuzzy `--title` retrieval, timeout logic, retry logic, or cache logic.
- Threaded cli.verbose through main orchestration, KeychainPassword, RealKeychainBackend, and OnePasswordBackend so backend selection and password-source diagnostics stay on stderr without exposing secrets.
 - Added SSHPASS_RS-prefixed eprintln! diagnostics around backend selection, standalone operations, keychain lookups, and 1Password command execution/status while keeping stdin secret data out of logs.

# Make -v (--verbose) available for all commands

## TL;DR

> **Quick Summary**: Thread the `-v` flag through ALL sshpass-rs code paths so users can see which backend is selected, what `op` commands run, and how password resolution proceeds — enabling debugging of issues like "SSHPASS_RS_BACKEND=op set but still queries keychain".
> 
> **Deliverables**:
> - `verbose: bool` threaded through `build_keychain_backend()`, `resolve_password()`, `handle_standalone()`
> - `verbose` field added to `OnePasswordBackend` and `RealKeychainBackend`
> - ~12 `eprintln!("SSHPASS_RS: ...")` calls at strategic diagnostic points
> - Integration tests verifying verbose output on stderr
> - Warning for unrecognized `SSHPASS_RS_BACKEND` values
> 
> **Estimated Effort**: Short
> **Parallel Execution**: YES - 2 waves
> **Critical Path**: Task 1 → Task 2 → Task 3

---

## Context

### Original Request
User set `SSHPASS_RS_BACKEND=op` environment variable but reports sshpass-rs still queries from keychain. Without verbose output, there's no way to diagnose the issue. User wants `-v` to work for all commands including standalone operations (`--list`, `--store`, `--delete`).

### Interview Summary
**Key Discussions**:
- Full diagnostic info desired: backend selection, op commands, env vars, results
- Show actual `op` commands being run (never show passwords/stdin_data)
- Use `eprintln!` to stderr with `SSHPASS_RS: ` prefix

### Metis Review
**Identified Gaps** (all addressed):
- `build_keychain_backend()` called TWICE (line 83 via manager, line 161 in resolve_password) — both need verbose
- `RealKeychainBackend` is currently a unit struct — needs verbose field too for "querying OS keychain" diagnostics
- Existing PTY verbose uses `"SSHPASS "` prefix — new messages use `"SSHPASS_RS: "` to distinguish
- Unrecognized `SSHPASS_RS_BACKEND` values silently fall through — should warn in verbose
- Never print `stdin_data`, `SecretString`, or password values in verbose output

---

## Work Objectives

### Core Objective
Thread `verbose: bool` through all sshpass-rs orchestration functions and backend structs, adding diagnostic `eprintln!` output at every key decision point.

### Concrete Deliverables
- Modified: `src/main.rs` — thread verbose through 3 orchestration functions
- Modified: `src/onepassword.rs` — add `verbose` field, log op commands
- Modified: `src/keychain.rs` — add `verbose` field to `RealKeychainBackend`
- Modified: `src/password.rs` — log password source and keychain outcomes
- New tests in `tests/integration.rs` and `tests/onepassword_integration.rs`

### Definition of Done
- [ ] `cargo test` — all tests pass (existing + new)
- [ ] `cargo clippy -- -D warnings` — no warnings
- [ ] `sshpass-rs -v -p test echo hi` shows `SSHPASS_RS:` messages on stderr
- [ ] `sshpass-rs -v --list` shows backend selection on stderr
- [ ] `SSHPASS_RS_BACKEND=op sshpass-rs -v --list` shows 1Password backend info

### Must Have
- `verbose: bool` parameter on `build_keychain_backend(verbose)`
- `verbose: bool` field on `OnePasswordBackend` and `RealKeychainBackend`
- Verbose output at backend selection: "selected 1Password backend" / "selected OS keychain backend"
- Verbose output for op commands: full args (never stdin_data)
- Verbose output for password resolution: which source selected
- Verbose output for standalone ops: operation + key + result
- Verbose output for keychain get: hit/miss/error
- Warning for unrecognized `SSHPASS_RS_BACKEND` values
- Verbose for standalone commands (`--list`, `--store`, `--delete`)

### Must NOT Have (Guardrails)
- ❌ Passwords/secrets in verbose output — SECURITY VIOLATION
- ❌ `stdin_data` content from `run_op_with_stdin()` in logs
- ❌ Changes to existing PTY verbose messages (`"SSHPASS searching..."` etc.)
- ❌ Logging framework (`log`, `tracing`) — use `eprintln!`
- ❌ Multiple verbosity levels (`-vv`)
- ❌ Colored output
- ❌ Changes to exit codes or stdout based on verbose
- ❌ Verbose in test-only backends (`FileKeychainBackend`, `InMemoryKeychainBackend`)
- ❌ Verbose for signal handling, PTY setup, or matcher internals

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: YES (tests-after)
- **Framework**: `cargo test` + `assert_cmd` + `predicates`

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — core implementation):
├── Task 1: Thread verbose through main.rs orchestration + backend structs [deep]
└── Task 2: Add verbose logging to OnePasswordBackend and RealKeychainBackend [unspecified-high]

Wave 2 (After Wave 1 — tests):
└── Task 3: Add integration tests for verbose output [unspecified-high]

Wave FINAL (After ALL tasks):
├── Task F1: Code quality + full test suite [unspecified-high]
└── Task F2: Manual QA — run with -v, verify output [unspecified-high]
-> Present results -> Get explicit user okay
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 | — | 2, 3 |
| 2 | 1 | 3 |
| 3 | 1, 2 | — |

Critical Path: Task 1 → Task 2 → Task 3 → F1+F2 → user okay

---

## TODOs

- [x] 1. Thread verbose through main.rs orchestration and backend struct signatures

  **What to do**:
  - Modify `build_keychain_backend()` signature to `fn build_keychain_backend(verbose: bool) -> Box<dyn KeychainBackend>`
  - Update both call sites:
    - `build_keychain_manager()` → needs `verbose` param: `fn build_keychain_manager(verbose: bool) -> KeychainManager`
    - `resolve_password()` call at line 161
  - Update `run()` to pass `cli.verbose` to `build_keychain_manager(cli.verbose)` and to `resolve_password(&cli)` (resolve_password needs access to verbose)
  - Modify `handle_standalone()` to accept `verbose: bool` and pass it down
  - Modify `resolve_password()` to accept `verbose: bool` (or get it from cli)
  - Add `verbose: bool` field to `OnePasswordBackend` struct — update `new()` and `with_op_path()` constructors
  - Add `verbose: bool` field to `RealKeychainBackend` — change from unit struct to `pub struct RealKeychainBackend { verbose: bool }`. Update constructor.
  - In `build_keychain_backend(verbose)`, add verbose logging:
    ```rust
    if verbose {
        // Log env var check
        eprintln!("SSHPASS_RS: checking SSHPASS_RS_BACKEND environment variable");
    }
    // After deciding backend:
    if verbose {
        eprintln!("SSHPASS_RS: selected 1Password backend (vault: {})", vault.as_deref().unwrap_or("default"));
        // or
        eprintln!("SSHPASS_RS: selected OS keychain backend");
    }
    // For unknown values:
    if verbose {
        eprintln!("SSHPASS_RS: unknown backend '{}', falling back to OS keychain", backend);
    }
    ```
  - In `resolve_password()`, add verbose logging for which source:
    ```rust
    if verbose { eprintln!("SSHPASS_RS: using password from -p argument"); }
    if verbose { eprintln!("SSHPASS_RS: using password from file '{}'", filename); }
    if verbose { eprintln!("SSHPASS_RS: using keychain with key '{}'", key); }
    // etc.
    ```
  - In `handle_standalone()`, add logging for operation:
    ```rust
    if verbose { eprintln!("SSHPASS_RS: performing store for key '{}'", key); }
    if verbose { eprintln!("SSHPASS_RS: performing delete for key '{}'", key); }
    if verbose { eprintln!("SSHPASS_RS: listing stored keys"); }
    ```
  - Update `KeychainPassword::new()` to accept `verbose: bool` and store it
  - In `KeychainPassword::resolve()`, add logging:
    ```rust
    if self.verbose { eprintln!("SSHPASS_RS: querying backend for key '{}'", self.key); }
    // On hit: eprintln!("SSHPASS_RS: key '{}' found in backend", self.key);
    // On key-not-found: eprintln!("SSHPASS_RS: key '{}' not found, falling back to interactive prompt", self.key);
    // On error: eprintln!("SSHPASS_RS: backend error: {}", msg);
    ```
  - Pass `verbose` through `PasswordResolver::resolve_with_keychain()` to `KeychainPassword`

  **Must NOT do**:
  - Do NOT change the `KeychainBackend` trait method signatures
  - Do NOT change existing PTY verbose messages
  - Do NOT add verbose to `FileKeychainBackend` or `InMemoryKeychainBackend`
  - Do NOT change exit codes or stdout based on verbose
  - Do NOT print passwords or secrets in verbose output

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Threading a parameter through multiple interconnected functions requires careful tracking of all call sites
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 1 (sequential with Task 2)
  - **Blocks**: Tasks 2, 3
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/pty.rs:138-183` — Existing verbose pattern: `if verbose { eprintln!("SSHPASS ..."); }`
  - `src/main.rs:82-104` — `build_keychain_manager()` and `build_keychain_backend()` — the functions to modify
  - `src/main.rs:114-129` — `handle_standalone()` — needs verbose param
  - `src/main.rs:138-163` — `resolve_password()` — needs verbose param
  - `src/password.rs:120-182` — `KeychainPassword` struct and `resolve()` — needs verbose field
  - `src/password.rs:184-220` — `PasswordResolver` enum and `resolve_with_keychain()` — needs verbose threading
  - `src/onepassword.rs:118-137` — `OnePasswordBackend` struct and constructors — needs verbose field
  - `src/keychain.rs:18` — `RealKeychainBackend` unit struct — needs verbose field

  **WHY Each Reference Matters**:
  - `src/pty.rs:138-183` — This is THE pattern to follow for verbose logging
  - `src/main.rs:82-104` — Both `build_keychain_manager()` and `build_keychain_backend()` are called from `run()` — must thread verbose through both
  - `src/password.rs:120-182` — `KeychainPassword` has the resolve() method that needs to log hit/miss/error
  - `src/keychain.rs:18` — Changing from unit struct requires updating all `RealKeychainBackend` usages

  **Acceptance Criteria**:
  - [ ] `build_keychain_backend(verbose: bool)` compiles
  - [ ] Both call sites updated (manager + resolve_password)
  - [ ] `OnePasswordBackend` has `verbose` field
  - [ ] `RealKeychainBackend` has `verbose` field
  - [ ] `resolve_password` logs password source selection
  - [ ] `handle_standalone` logs operation
  - [ ] `KeychainPassword::resolve()` logs hit/miss/error
  - [ ] `cargo check` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Verbose backend selection output
    Tool: Bash
    Preconditions: cargo build succeeds
    Steps:
      1. Run `SSHPASS_RS_BACKEND=op ./target/debug/sshpass-rs -v --list 2>&1`
      2. Assert stderr contains "SSHPASS_RS:"
      3. Assert stderr contains "1Password" or "backend"
    Expected Result: Verbose messages on stderr showing backend selection
    Failure Indicators: No SSHPASS_RS: messages on stderr
    Evidence: .sisyphus/evidence/task-1-verbose-backend.txt

  Scenario: No secrets leaked in verbose output
    Tool: Bash
    Preconditions: cargo build succeeds
    Steps:
      1. Run `./target/debug/sshpass-rs -v -p "SUPERSECRET123" target/debug/fake_ssh --mode success 2>/tmp/verbose_stderr.txt`
      2. Run `grep "SUPERSECRET123" /tmp/verbose_stderr.txt`
      3. Assert grep finds nothing (exit code 1)
    Expected Result: Password never appears in verbose stderr
    Failure Indicators: Password found in stderr output
    Evidence: .sisyphus/evidence/task-1-no-secret-leak.txt
  ```

  **Commit**: YES
  - Message: `feat(verbose): thread -v flag through all orchestration paths with diagnostic output`
  - Files: `src/main.rs`, `src/password.rs`, `src/keychain.rs`, `src/onepassword.rs`
  - Pre-commit: `cargo check`

- [x] 2. Add verbose logging to OnePasswordBackend and RealKeychainBackend operations

  **What to do**:
  - In `OnePasswordBackend::run_op()`, add verbose logging:
    ```rust
    if self.verbose {
        eprintln!("SSHPASS_RS: running: op {}", args.join(" "));
    }
    // After execution:
    if self.verbose {
        eprintln!("SSHPASS_RS: op exited with status {}", output.status);
    }
    ```
  - In `OnePasswordBackend::run_op_with_stdin()`, add verbose logging:
    ```rust
    if self.verbose {
        eprintln!("SSHPASS_RS: running: op {} (with stdin data)", args.join(" "));
    }
    // NEVER print stdin_data content!
    ```
  - In `OnePasswordBackend::store()`, log: `SSHPASS_RS: storing key '{}' in 1Password`
  - In `OnePasswordBackend::get()`, log: `SSHPASS_RS: looking up key '{}' in 1Password` and after list+filter: `SSHPASS_RS: found item id '{}' for key '{}'` or `SSHPASS_RS: key '{}' not found in 1Password`
  - In `OnePasswordBackend::delete()`, log: `SSHPASS_RS: deleting key '{}' from 1Password`
  - In `OnePasswordBackend::list()`, log: `SSHPASS_RS: listing keys from 1Password` and after: `SSHPASS_RS: found {} keys`
  - In `RealKeychainBackend::get()`, log: `SSHPASS_RS: querying OS keychain for key '{}'`
  - In `RealKeychainBackend::store()`, log: `SSHPASS_RS: storing key '{}' in OS keychain`
  - In `RealKeychainBackend::delete()`, log: `SSHPASS_RS: deleting key '{}' from OS keychain`
  - In `RealKeychainBackend::list()`, log: `SSHPASS_RS: listing keys from OS keychain`

  **Must NOT do**:
  - NEVER print `stdin_data` content (contains password JSON)
  - NEVER print `SecretString` or password values
  - Do NOT change the `KeychainBackend` trait
  - Do NOT add verbose to test backends (`InMemoryKeychainBackend`, `FileKeychainBackend`)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Adding verbose logging to multiple method implementations requires careful attention to security
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Task 1 struct changes)
  - **Parallel Group**: Wave 1 (after Task 1)
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/onepassword.rs:142-196` — `run_op()` and `run_op_with_stdin()` — add logging here
  - `src/onepassword.rs:199-261` — `KeychainBackend` impl — add logging to each method
  - `src/keychain.rs:51-104` — `RealKeychainBackend` impl — add logging to each method

  **WHY Each Reference Matters**:
  - `src/onepassword.rs:142-196` — These are where op CLI commands execute; logging the command args is critical for debugging
  - `src/keychain.rs:51-104` — Logging OS keychain access helps users confirm which backend is actually being used

  **Acceptance Criteria**:
  - [ ] `OnePasswordBackend` methods log op commands in verbose mode
  - [ ] `run_op_with_stdin()` logs args but NEVER stdin_data content
  - [ ] `RealKeychainBackend` methods log keychain operations in verbose mode
  - [ ] `cargo test` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: op commands shown in verbose output
    Tool: Bash
    Preconditions: Task 1 complete, mock_op in PATH
    Steps:
      1. Set up mock op in PATH (symlink)
      2. Run `SSHPASS_RS_BACKEND=op ./target/debug/sshpass-rs -v --list 2>&1`
      3. Assert stderr contains "SSHPASS_RS: running: op item list"
      4. Assert stderr contains "SSHPASS_RS:" messages about found keys
    Expected Result: Full op command visible in verbose stderr
    Failure Indicators: No op command in stderr
    Evidence: .sisyphus/evidence/task-2-op-commands.txt

  Scenario: stdin_data never printed
    Tool: Bash
    Preconditions: Task 1 complete, mock_op in PATH
    Steps:
      1. Set up mock op in PATH
      2. Run `SSHPASS_RS_BACKEND=op SSHPASS_RS_TEST_PASSWORD=mysecret ./target/debug/sshpass-rs -v --store test@host 2>/tmp/verbose_store.txt`
      3. Run `grep "mysecret" /tmp/verbose_store.txt`
      4. Assert grep finds nothing
    Expected Result: Password never appears in verbose output during store
    Failure Indicators: Password found in stderr
    Evidence: .sisyphus/evidence/task-2-no-stdin-leak.txt
  ```

  **Commit**: YES
  - Message: `feat(verbose): add diagnostic logging to 1Password and OS keychain backend operations`
  - Files: `src/onepassword.rs`, `src/keychain.rs`
  - Pre-commit: `cargo test`

- [x] 3. Add integration tests for verbose diagnostic output

  **What to do**:
  - Add new integration tests to `tests/integration.rs`:
    - `test_verbose_password_source_p_flag`: `-v -p testpass target/debug/fake_ssh --mode success` → stderr contains `"SSHPASS_RS: using password from -p argument"`
    - `test_verbose_backend_selection_default`: `-v --list` with `SSHPASS_RS_TEST_KEYCHAIN_FILE` → stderr contains `"SSHPASS_RS:.*keychain"`
    - `test_verbose_standalone_list`: `-v --list` → stderr contains `"SSHPASS_RS:.*listing"`
    - `test_verbose_no_secret_leak`: `-v -p SUPERSECRET target/debug/fake_ssh --mode success` → stderr does NOT contain "SUPERSECRET"
  - Add new integration tests to `tests/onepassword_integration.rs`:
    - `test_verbose_op_backend_selection`: With `SSHPASS_RS_BACKEND=op`, `-v --list` → stderr contains `"SSHPASS_RS:.*1Password"`
    - `test_verbose_op_command_shown`: With mock_op in PATH + `SSHPASS_RS_BACKEND=op`, `-v --list` → stderr contains `"SSHPASS_RS: running: op"`
    - `test_verbose_unknown_backend_warning`: With `SSHPASS_RS_BACKEND=invalid`, `-v --list` + test keychain file → stderr contains `"SSHPASS_RS:.*unknown backend"`

  **Must NOT do**:
  - Do NOT modify existing test functions
  - Do NOT break existing tests

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Integration tests with env var injection and stderr capture
  - **Skills**: [`test-driven-development`]

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on T1+T2)
  - **Parallel Group**: Wave 2
  - **Blocks**: None
  - **Blocked By**: Tasks 1, 2

  **References**:

  **Pattern References**:
  - `tests/integration.rs:142-149` — Existing verbose test pattern using `-v` flag and stderr assertions
  - `tests/integration.rs:216-241` — `temp_keychain_env()` helper and test isolation pattern
  - `tests/onepassword_integration.rs` — Existing 1Password integration tests with mock_op setup

  **WHY Each Reference Matters**:
  - `tests/integration.rs:142-149` — THE pattern for asserting verbose output on stderr
  - `tests/integration.rs:216-241` — Isolation pattern with temp keychain files

  **Acceptance Criteria**:
  - [ ] At least 7 new integration tests exist and pass
  - [ ] `cargo test` — full suite passes
  - [ ] Tests verify verbose output contains `"SSHPASS_RS:"` on stderr
  - [ ] Tests verify no secrets in verbose output

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: All integration tests pass
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test 2>&1`
      2. Assert exit code 0
      3. Assert 0 failures
    Expected Result: All tests pass including new verbose tests
    Failure Indicators: Any test failure
    Evidence: .sisyphus/evidence/task-3-tests.txt
  ```

  **Commit**: YES
  - Message: `test(verbose): add integration tests for diagnostic output across all command paths`
  - Files: `tests/integration.rs`, `tests/onepassword_integration.rs`
  - Pre-commit: `cargo test`

---

## Final Verification Wave (MANDATORY)

> 2 review agents run in PARALLEL. Both must APPROVE.

- [x] F1. **Code Quality + Full Test Suite** — `unspecified-high`
  Run `cargo check && cargo clippy -- -D warnings && cargo fmt --check && cargo test`. Review verbose output for security (no secrets). Zero regressions.
  Output: `Build [PASS/FAIL] | Tests [N pass/N fail] | VERDICT`

- [x] F2. **Manual QA — Run with -v** — `unspecified-high`
  Build binary. Test ALL verbose scenarios:
  1. `sshpass-rs -v -p test target/debug/fake_ssh --mode success` → shows password source + backend
  2. `sshpass-rs -v --list` with test keychain → shows backend + operation
  3. `SSHPASS_RS_BACKEND=op sshpass-rs -v --list` → shows 1Password backend selection
  4. `SSHPASS_RS_BACKEND=invalid sshpass-rs -v --list` with test keychain → shows warning
  5. Verify no secrets in any stderr output
  Output: `Scenarios [N/N pass] | VERDICT`

---

## Commit Strategy

| Commit | Message | Files | Pre-commit |
|--------|---------|-------|------------|
| 1 | `feat(verbose): thread -v flag through all orchestration paths with diagnostic output` | `src/main.rs`, `src/password.rs`, `src/keychain.rs`, `src/onepassword.rs` | `cargo check` |
| 2 | `feat(verbose): add diagnostic logging to 1Password and OS keychain backend operations` | `src/onepassword.rs`, `src/keychain.rs` | `cargo test` |
| 3 | `test(verbose): add integration tests for diagnostic output across all command paths` | `tests/integration.rs`, `tests/onepassword_integration.rs` | `cargo test` |

---

## Success Criteria

### Verification Commands
```bash
cargo check              # Expected: compiles cleanly
cargo test               # Expected: all tests pass
cargo clippy -- -D warnings  # Expected: no warnings
./target/debug/sshpass-rs -v -p test target/debug/fake_ssh --mode success 2>&1 | grep "SSHPASS_RS:"  # Expected: verbose output
```

### Final Checklist
- [ ] All "Must Have" present
- [ ] All "Must NOT Have" absent
- [ ] All tests pass
- [ ] Existing verbose PTY behavior unchanged

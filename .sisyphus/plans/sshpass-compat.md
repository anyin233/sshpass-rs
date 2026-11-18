# sshpass Behavioral Compatibility Alignment

## TL;DR

> **Quick Summary**: Make sshpassx behave identically to the original C sshpass for all non-sshpassx-specific features. Based on a line-by-line comparison of the original sshpass main.c against sshpassx Rust sources, 13 behavioral differences were identified and each will be fixed.
> 
> **Deliverables**:
> - Exit codes match original sshpass for all shared scenarios
> - Verbose output matches original sshpass message format
> - PTY/child process management matches original behavior
> - `-V` version flag support
> - Stdin/FD password reading deferred to prompt detection time
> - All existing tests updated, new compat tests added
> 
> **Estimated Effort**: Medium
> **Parallel Execution**: YES - 3 waves
> **Critical Path**: Task 1-4 (exit codes) → Task 5-8 (verbose/PTY) → Task 9-12 (advanced) → Final Verification

---

## Context

### Original Request
User asked to read the original sshpass source code from SourceForge and make sshpassx's non-specific behaviors match it as closely as possible.

### Interview Summary
**Key Discussions**:
- Full source comparison between original sshpass (kevinburke/sshpass mirror, main.c ~400 lines) and sshpassx Rust implementation (~3000 lines across 9 modules)
- 13+ behavioral differences identified spanning exit codes, verbose output, PTY management, and CLI behavior

**Research Findings**:
- Original sshpass has exit code 4 (RETURN_PARSE_ERRROR) defined but **never used**
- Original explicitly does NOT detect HOST_KEY_CHANGED (commented out code with note: "sent to stderr, not the tty")
- Original does NOT clear SSHPASS env var — sshpassx does (security improvement)
- Original reads stdin/fd password **lazily** at prompt match time, not eagerly before spawn

### Metis Review
**Identified Gaps** (addressed):
- SSHPASS env var clearing (Diff #14): Keep sshpassx behavior as intentional security improvement
- HOST_KEY_CHANGED: Keep detection as enhancement (exit 7), since it's useful and doesn't conflict
- argv obfuscation: Skip for now — Rust makes this non-trivial and `-p` is already documented as insecure
- Lazy password reading: Implement as architectural change with `DeferredPassword` pattern

---

## Work Objectives

### Core Objective
Align sshpassx with the original sshpass behavior for all shared functionality, so it serves as a true drop-in replacement.

### Concrete Deliverables
- `cli.rs`: Add `-V` flag, fix no-args behavior (show help + exit 0), validate `-e` early
- `password.rs` / `main.rs`: Defer stdin/fd password reading to prompt detection time
- `pty.rs`: Fix exit codes (signal-killed → 255, remove ParseError), change child termination to PTY close, add verbose messages
- `error.rs`: Add `-e` specific error variant mapping
- `src/bin/fake_ssh.rs`: Add `signal-exit` test mode
- `tests/integration.rs`: Update existing tests, add new compat tests
- `tests/pty_integration.rs`: Update existing tests, add verbose output tests

### Definition of Done
- [ ] `cargo test` passes with zero failures
- [ ] `cargo clippy` clean (no new warnings)
- [ ] All 13 behavioral differences resolved or documented as intentional deviations
- [ ] Exit codes match original sshpass for: no args, -e without SSHPASS, conflicting sources, signal-killed child, wrong password, host key unknown, child exit passthrough
- [ ] Verbose output matches original format: "SSHPASS read:", "SSHPASS detected prompt. Sending password.", etc.

### Must Have
- Exit code 0 when no command given (match original `show_help(); return 0;`)
- Exit code 1 when `-e` used without SSHPASS env var (match original `RETURN_INVALID_ARGUMENTS`)
- Exit code 255 for signal-killed children (match original)
- Child exit code passthrough when no prompt matched (remove ParseError exit 4 logic)
- "SSHPASS read: ..." verbose output for PTY reads
- Verbose message wording matching original exactly
- PTY close for child termination instead of SIGKILL
- `-V` version flag
- Deferred stdin/fd password reading

### Must NOT Have (Guardrails)
- **DO NOT** touch `keychain.rs`, `onepassword.rs`, or sshpassx-specific feature code (-k, --store, --delete, --list, SSH alias resolution, 1Password backend)
- **DO NOT** add new crate dependencies without justification
- **DO NOT** refactor code structure beyond what's needed for the specific behavioral change
- **DO NOT** change the PTY spawn mechanism (normal vs legacy paths in `pty.rs`)
- **DO NOT** remove HOST_KEY_CHANGED detection (keep as enhancement)
- **DO NOT** stop clearing SSHPASS env var (keep as security improvement)
- **DO NOT** implement argv obfuscation for -p (too complex for low value in Rust)

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: YES (Tests-after — update existing tests, add new ones alongside changes)
- **Framework**: cargo test + assert_cmd + predicates + fake_ssh binary
- **Pattern**: Each behavioral change gets at least one integration test asserting exact exit code and/or stderr content

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **CLI**: Use Bash (cargo test) — Run tests, assert exit codes, check stderr
- **PTY**: Use integration tests — Verify PTY behavior through fake_ssh modes

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — exit code fixes, independent of each other):
├── Task 1: Fix -e without SSHPASS → exit 1 [quick]
├── Task 2: Fix no-args → show help + exit 0 [quick]
├── Task 3: Fix signal-killed child → exit 255 [quick]
├── Task 4: Remove ParseError exit 4 → passthrough child exit code [quick]
├── Task 5: Add -V version flag [quick]
└── Task 6: Add fake_ssh signal-exit mode (needed by Task 3 tests) [quick]

Wave 2 (After Wave 1 — verbose output + PTY fixes):
├── Task 7: Add "SSHPASS read:" verbose messages [quick]
├── Task 8: Match original verbose message wording [quick]
├── Task 9: Add verbose messages for wrong password + host key detection [quick]
└── Task 10: Use PTY close instead of SIGKILL for child termination [deep]

Wave 3 (After Wave 2 — architectural change):
└── Task 11: Defer stdin/fd password reading to prompt detection time [deep]

Wave FINAL (After ALL tasks — 4 parallel reviews):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Real QA run (unspecified-high)
└── Task F4: Scope fidelity check (deep)
-> Present results -> Get explicit user okay
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 | — | F1-F4 |
| 2 | — | F1-F4 |
| 3 | 6 | F1-F4 |
| 4 | — | F1-F4 |
| 5 | — | F1-F4 |
| 6 | — | 3 |
| 7 | — | F1-F4 |
| 8 | — | F1-F4 |
| 9 | — | F1-F4 |
| 10 | — | 11, F1-F4 |
| 11 | 10 | F1-F4 |

### Agent Dispatch Summary

- **Wave 1**: **6 tasks** — T1-T6 → `quick`
- **Wave 2**: **4 tasks** — T7-T9 → `quick`, T10 → `deep`
- **Wave 3**: **1 task** — T11 → `deep`
- **FINAL**: **4 tasks** — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [x] 1. Fix -e without SSHPASS → exit 1 (InvalidArguments)

  **What to do**:
  - Currently, when `-e` is used without the `SSHPASS` env var being set, the error is detected in `password.rs` `EnvPassword::resolve()` which returns `SshpassError::PasswordSource(...)`, and `SshpassExitCode::from(&err)` maps ALL `PasswordSource` errors to exit 3 (RuntimeError).
  - The original sshpass detects this during CLI parsing (`parse_options()`) and returns `RETURN_INVALID_ARGUMENTS` (1).
  - **Fix approach**: Move the `-e` + missing SSHPASS validation into `cli.rs` `Cli::validate()`. When `use_env` is true, check `std::env::var("SSHPASS")`. If not set, return error with exit code 1 and message `"sshpass: -e option given but SSHPASS environment variable not set"`.
  - Update the existing test `env_flag_without_sshpass_exits_three` in `tests/integration.rs` to assert exit code 1 instead of 3, and update the stderr assertion to match the new message.

  **Must NOT do**:
  - Do not touch `keychain.rs`, `onepassword.rs`, or any sshpassx-specific code.
  - Do not change the behavior of `-e` when SSHPASS IS set (that should continue working as before).

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3, 4, 5, 6)
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/cli.rs:288-312` — `Cli::validate()` method where mutual exclusivity check happens. Add SSHPASS env check here.
  - `src/password.rs:93-103` — Current `EnvPassword::resolve()` where SSHPASS is checked at runtime. The env check should move to validation.
  - Original sshpass main.c line 136-143 — The `-e` case in `parse_options()` that checks SSHPASS and sets `error=RETURN_INVALID_ARGUMENTS`.

  **Test References**:
  - `tests/integration.rs:306-315` — Existing test `env_flag_without_sshpass_exits_three` that needs updating to assert exit code 1.

  **WHY Each Reference Matters**:
  - `cli.rs:validate()` is where early validation happens — this is the correct place to add env var checking, matching the original's approach of validating during option parsing.
  - `password.rs:EnvPassword::resolve()` still needs to handle the case but should no longer be the primary error path for missing SSHPASS.
  - The original source shows the exact error message format: `"sshpass: -e option given but SSHPASS environment variable not set\n"`.

  **Acceptance Criteria**:
  - [ ] `sshpassx -e <command>` (without SSHPASS set) exits with code 1
  - [ ] stderr contains `"sshpass: -e option given but SSHPASS environment variable not set"` (or close equivalent using sshpassx prefix)
  - [ ] `SSHPASS=pass sshpassx -e <command>` still works normally

  **QA Scenarios**:

  ```
  Scenario: -e without SSHPASS exits 1
    Tool: Bash (cargo test)
    Preconditions: SSHPASS env var is NOT set
    Steps:
      1. Run: cargo test env_flag_without_sshpass -- --nocapture
      2. Assert test passes
      3. Run: cargo test --test integration env_flag_without_sshpass -- --nocapture
      4. Assert test passes
    Expected Result: Test asserts exit code 1 and stderr contains "-e option given but SSHPASS environment variable not set"
    Evidence: .sisyphus/evidence/task-1-env-flag-exit-code.txt

  Scenario: -e WITH SSHPASS still works
    Tool: Bash (cargo test)
    Preconditions: Build fake_ssh first
    Steps:
      1. Run: cargo test env_flag_success -- --nocapture
      2. Assert test passes (exit 0, stdout contains "Welcome")
    Expected Result: Existing test continues to pass
    Evidence: .sisyphus/evidence/task-1-env-flag-success.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): exit 1 when -e used without SSHPASS env var`
  - Files: `src/cli.rs`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 2. Fix no-arguments behavior → show help + exit 0

  **What to do**:
  - Currently, when no arguments are given (or a password flag is given but no command), `Cli::validate()` returns exit code 1 with "missing wrapped command".
  - The original sshpass shows help text and exits 0 in this case: `if(argc-opt_offset<1) { show_help(); return 0; }`.
  - **Fix approach**: In `cli.rs` `Cli::validate()`, change the "no command" check: instead of returning an error, set a flag (or adjust the logic in `main.rs`) to show help and exit 0. The cleanest approach is to change `validate()` to NOT error on missing command, and in `main.rs` after validation, check if command is empty (and not standalone, not help) → call `cli.print_help()` and return 0.
  - Update existing test `missing_command_exits_one` to assert exit 0 and help text instead.
  - Add new test: `no_arguments_shows_help_exits_zero` for the bare `sshpassx` invocation.

  **Must NOT do**:
  - Do not change standalone operations behavior (--store, --delete, --list should still work without a command).
  - Do not change -h/--help behavior.

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 3, 4, 5, 6)
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/cli.rs:307-309` — Current "missing wrapped command" validation that returns exit 1.
  - `src/main.rs:38-41` — Current help flag handling. Similar logic needed for "no command" case.
  - Original sshpass main.c lines 177-182 — `if(argc-opt_offset<1) { show_help(); return 0; }`

  **Test References**:
  - `tests/integration.rs:89-98` — Existing test `missing_command_exits_one` to update.
  - `src/main.rs:349-354` — Existing in-module test `conflicting_password_sources_exit_with_code_two` showing how exit code tests work.

  **WHY Each Reference Matters**:
  - `cli.rs:307-309` is the exact line that needs to change — remove the error return.
  - `main.rs:38-41` shows the pattern for "display help and exit 0" that should be replicated for the no-command case.
  - The original source confirms: no command → show help → return 0, NOT an error.

  **Acceptance Criteria**:
  - [ ] `sshpassx` (no args) shows help text and exits 0
  - [ ] `sshpassx -p pass` (with flag but no command) shows help text and exits 0
  - [ ] Standalone operations (--store, --list, --delete) still work without a command

  **QA Scenarios**:

  ```
  Scenario: No arguments shows help and exits 0
    Tool: Bash (cargo test)
    Preconditions: None
    Steps:
      1. Run: cargo test --test integration no_arguments_shows_help -- --nocapture (new test)
      2. Assert: exit code 0, stdout contains "Usage:" or "sshpassx"
    Expected Result: Help text shown, exit 0
    Evidence: .sisyphus/evidence/task-2-no-args-help.txt

  Scenario: -p with no command shows help and exits 0
    Tool: Bash (cargo test)
    Preconditions: None
    Steps:
      1. Run: cargo test --test integration missing_command -- --nocapture (updated test)
      2. Assert: exit code 0, stdout contains help text
    Expected Result: Help text shown, exit 0 (previously was exit 1)
    Evidence: .sisyphus/evidence/task-2-flag-no-command.txt

  Scenario: Standalone ops still work
    Tool: Bash (cargo test)
    Preconditions: None
    Steps:
      1. Run: cargo test --test integration keychain_list_empty -- --nocapture
      2. Assert: test passes
    Expected Result: --list still works, exits 0 with "(empty)"
    Evidence: .sisyphus/evidence/task-2-standalone-unaffected.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): show help and exit 0 when no command given`
  - Files: `src/cli.rs`, `src/main.rs`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 3. Fix signal-killed child → exit 255

  **What to do**:
  - Currently `exit_code_from_status()` in `pty.rs:366-370` returns `128 + signal` when the child is killed by a signal.
  - The original sshpass returns 255: `else return 255;` (when `!WIFEXITED && WIFSIGNALED`).
  - **Fix**: Change `exit_code_from_status()` to return 255 when the exit status has no code (i.e., child was signaled): `status.code().unwrap_or(255)`.
  - Add new integration test using the `signal-exit` fake_ssh mode (Task 6) that verifies sshpassx returns 255.

  **Must NOT do**:
  - Do not change the handling of normal (non-signaled) exits.

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: F1-F4
  - **Blocked By**: Task 6 (needs signal-exit fake_ssh mode for testing)

  **References**:

  **Pattern References**:
  - `src/pty.rs:366-370` — `exit_code_from_status()` function to modify. Currently: `status.code().unwrap_or_else(|| status.signal().map_or(1, |signal| 128 + signal))`.
  - Original sshpass main.c lines 193-197 — `if(WIFEXITED(status)) return WEXITSTATUS(status); else return 255;`

  **Test References**:
  - `tests/integration.rs:222-236` — Existing `child_exit_code_passthrough` test showing the pattern for exit code testing.

  **Acceptance Criteria**:
  - [ ] When wrapped child is killed by a signal, sshpassx returns 255
  - [ ] Normal exit code passthrough still works (child exits 42 → sshpassx exits 42)

  **QA Scenarios**:

  ```
  Scenario: Signal-killed child returns 255
    Tool: Bash (cargo test)
    Preconditions: fake_ssh binary built with signal-exit mode (Task 6)
    Steps:
      1. Run: cargo test --test integration signal_killed_child -- --nocapture
      2. Assert: test passes, exit code is 255
    Expected Result: sshpassx returns 255 when child killed by signal
    Evidence: .sisyphus/evidence/task-3-signal-exit-255.txt

  Scenario: Normal exit code still passes through
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration child_exit_code_passthrough -- --nocapture
      2. Assert: test still passes (exit 42)
    Expected Result: Existing behavior preserved
    Evidence: .sisyphus/evidence/task-3-normal-exit-passthrough.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): return 255 for signal-killed child processes`
  - Files: `src/pty.rs`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 4. Remove ParseError exit 4 → pass through child exit code

  **What to do**:
  - Currently `finalize_child_exit()` in `pty.rs:356-364` returns exit code 4 (ParseError) when the child exits non-zero AND no password prompt was matched.
  - The original sshpass simply returns `WEXITSTATUS(status)` — it never uses exit code 4.
  - **Fix**: Remove the ParseError mapping in `finalize_child_exit()`. Just return the child's exit code directly. The function becomes: `fn finalize_child_exit(status: ExitStatus, _saw_match: bool) -> i32 { exit_code_from_status(status) }`. Or simplify by inlining.
  - Update existing test `parse_error_exits_four` in `tests/integration.rs` — it should now assert the child's actual exit code (1, since fake_ssh `parse-error` mode exits 1).
  - Update `tests/pty_integration.rs` test `test_parse_error` similarly.

  **Must NOT do**:
  - Do not change the `SshpassExitCode::ParseError` enum variant (it's still a valid concept, just not auto-triggered).
  - Do not change error handling for actual sshpassx errors.

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/pty.rs:356-364` — `finalize_child_exit()` function with the ParseError logic to remove.
  - Original sshpass main.c lines 190-197 — The exit handling that just returns WEXITSTATUS or 255.
  - `src/bin/fake_ssh.rs:137-148` — `parse-error` mode exits 1, so test should now assert exit 1.

  **Test References**:
  - `tests/integration.rs:239-246` — `parse_error_exits_four` test to update to assert code(1).
  - `tests/pty_integration.rs:178-182` — `test_parse_error` test to update to assert exit 1.

  **Acceptance Criteria**:
  - [ ] When child exits 1 without prompt match, sshpassx returns 1 (not 4)
  - [ ] When child exits 0 without prompt match, sshpassx returns 0
  - [ ] Wrong password detection (exit 5) still works
  - [ ] Host key detection (exit 6, 7) still works

  **QA Scenarios**:

  ```
  Scenario: Child exit code passes through without ParseError mapping
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration parse_error -- --nocapture
      2. Assert: test passes with new exit code assertion (1 instead of 4)
    Expected Result: sshpassx returns child's actual exit code (1)
    Evidence: .sisyphus/evidence/task-4-no-parse-error.txt

  Scenario: Wrong password still returns exit 5
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration wrong_password -- --nocapture
      2. Assert: test passes (exit 5)
    Expected Result: Wrong password detection unaffected
    Evidence: .sisyphus/evidence/task-4-wrong-password-unchanged.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): pass through child exit code without ParseError mapping`
  - Files: `src/pty.rs`, `tests/integration.rs`, `tests/pty_integration.rs`
  - Pre-commit: `cargo test`

- [x] 5. Add -V version flag

  **What to do**:
  - The original sshpass supports `-V` to print version and exit 0: `printf("%s\n(C) 2006-2011 Lingnu...\nUsing \"%s\" as the default password prompt indicator.\n", PACKAGE_STRING, PASSWORD_PROMPT); exit(0);`
  - sshpassx currently has no `-V` flag.
  - **Fix**: Add `-V` flag to `Cli` struct in `cli.rs`. In `main.rs`, check for it early (before keychain manager creation) and print version info + exit 0.
  - Version string format: `"sshpassx {version}\n(C) ...\nUsing \"{prompt}\" as the default password prompt indicator.\n"` using the Cargo.toml version.
  - Add integration test: `version_flag_prints_version_exits_zero`.

  **Must NOT do**:
  - Do not conflict with clap's built-in version handling (we use `disable_version_flag = true`).

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/cli.rs:6` — `#[command(disable_help_flag = true, disable_version_flag = true)]` — version flag already disabled for clap, we manage it manually.
  - `src/cli.rs:41-42` — How `-h` is defined: `#[arg(long = "help", short = 'h')]` — follow same pattern for `-V`.
  - `src/main.rs:38-41` — How help is handled early in `run()` — follow same pattern for version.
  - Original sshpass main.c lines 153-161 — `-V` handler printing PACKAGE_STRING and PASSWORD_PROMPT.

  **Acceptance Criteria**:
  - [ ] `sshpassx -V` prints version string and exits 0
  - [ ] Output includes "sshpassx" and the version number
  - [ ] Output includes the default password prompt indicator

  **QA Scenarios**:

  ```
  Scenario: -V prints version and exits 0
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration version_flag -- --nocapture
      2. Assert: test passes, exit 0, stdout contains "sshpassx" and version
    Expected Result: Version info printed, exit 0
    Evidence: .sisyphus/evidence/task-5-version-flag.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): add -V version flag`
  - Files: `src/cli.rs`, `src/main.rs`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 6. Add signal-exit mode to fake_ssh test helper

  **What to do**:
  - Add a new mode `"signal-exit"` to `src/bin/fake_ssh.rs` that:
    1. Prints "Password: " to stderr
    2. Reads a line from stdin
    3. Kills itself with SIGTERM: `unsafe { libc::kill(libc::getpid(), libc::SIGTERM) }`
  - This mode is needed by Task 3's tests to verify sshpassx returns 255 for signal-killed children.

  **Must NOT do**:
  - Do not modify existing fake_ssh modes.

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1
  - **Blocks**: Task 3
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/bin/fake_ssh.rs:30-163` — Existing mode implementations. Follow the same pattern.
  - `src/bin/fake_ssh.rs:34-46` — "success" mode as template for prompt → read → action.

  **Acceptance Criteria**:
  - [ ] `fake_ssh --mode signal-exit` prints password prompt, reads input, then dies by signal
  - [ ] The process exit status shows it was killed by SIGTERM (not a normal exit)

  **QA Scenarios**:

  ```
  Scenario: fake_ssh signal-exit mode works
    Tool: Bash
    Steps:
      1. Run: echo "test" | cargo run --bin fake_ssh -- --mode signal-exit; echo "Exit: $?"
      2. Assert: exit code is non-zero (128+15=143 or similar signal exit)
    Expected Result: Process killed by signal
    Evidence: .sisyphus/evidence/task-6-signal-exit-mode.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): add signal-exit mode to fake_ssh test helper`
  - Files: `src/bin/fake_ssh.rs`
  - Pre-commit: `cargo test`

- [x] 7. Add "SSHPASS read:" verbose messages for PTY output

  **What to do**:
  - The original sshpass prints `"SSHPASS read: %s\n"` to stderr in verbose mode after every PTY read, showing the raw buffer content.
  - sshpassx currently does NOT print PTY read output in verbose mode.
  - **Fix**: In `pty.rs` `run_with_password()`, after the `read_retrying()` call that reads from the master PTY, add: `if verbose { eprintln!("SSHPASS read: {}", String::from_utf8_lossy(output)); }`.
  - Add a test that captures stderr and verifies "SSHPASS read:" appears in verbose mode.

  **Must NOT do**:
  - Do not print the password itself in verbose output.
  - Do not change non-verbose behavior.

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2 (with Tasks 8, 9, 10)
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/pty.rs:160-165` — PTY read section in `run_with_password()`. Add verbose logging after `let output = &buffer[..count];`.
  - Original sshpass main.c lines 263-266 — `if(args.verbose) { fprintf(stderr, "SSHPASS read: %s\n", buffer); }` right after `numread=read(fd, buffer, sizeof(buffer)-1); buffer[numread] = '\0';`.

  **Acceptance Criteria**:
  - [ ] In verbose mode, stderr contains "SSHPASS read:" followed by PTY output content
  - [ ] Non-verbose mode does not print "SSHPASS read:"

  **QA Scenarios**:

  ```
  Scenario: Verbose mode shows SSHPASS read messages
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration verbose_shows_sshpass_read -- --nocapture (new test)
      2. Assert: test passes, stderr contains "SSHPASS read:"
    Expected Result: "SSHPASS read:" visible in verbose stderr
    Evidence: .sisyphus/evidence/task-7-verbose-read.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): add SSHPASS read verbose messages`
  - Files: `src/pty.rs`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 8. Match original sshpass verbose message wording

  **What to do**:
  - Currently sshpassx prints:
    - `"SSHPASS detected password prompt"` (line 185)
    - `"SSHPASS sending password"` (line 186)
  - Original sshpass prints:
    - `"SSHPASS detected prompt. Sending password.\n"` (single combined message)
  - **Fix**: Replace the two separate messages with one combined message matching the original format exactly: `"SSHPASS detected prompt. Sending password.\n"`.
  - Also add verbose `firsttime` behavior: the original only prints "SSHPASS searching for password prompt using match..." once (first time). sshpassx already does this correctly.
  - Update existing test `verbose_shows_detected_and_sending` in `tests/integration.rs` to assert the new combined message.

  **Must NOT do**:
  - Do not change the "SSHPASS searching for password prompt using match" message (it already matches).

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/pty.rs:184-187` — Current two-line verbose messages to combine into one.
  - Original sshpass main.c lines 270-272 — `if(args.verbose) fprintf(stderr, "SSHPASS detected prompt. Sending password.\n");`

  **Test References**:
  - `tests/integration.rs:369-378` — `verbose_shows_detected_and_sending` test to update.

  **Acceptance Criteria**:
  - [ ] Verbose output contains `"SSHPASS detected prompt. Sending password."` as one message
  - [ ] No longer contains separate "SSHPASS detected password prompt" / "SSHPASS sending password" messages

  **QA Scenarios**:

  ```
  Scenario: Combined verbose message matches original
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration verbose_shows_detected_and_sending -- --nocapture
      2. Assert: stderr contains "SSHPASS detected prompt. Sending password."
    Expected Result: Single combined message matching original
    Evidence: .sisyphus/evidence/task-8-verbose-combined.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): match original sshpass verbose message wording`
  - Files: `src/pty.rs`, `tests/integration.rs`, `tests/pty_integration.rs`
  - Pre-commit: `cargo test`

- [x] 9. Add verbose messages for wrong password and host key detection

  **What to do**:
  - Original sshpass prints these messages in verbose mode that sshpassx is missing:
    - Wrong password: `"SSHPASS detected prompt, again. Wrong password. Terminating.\n"`
    - Host key unknown: `"SSHPASS detected host authentication prompt. Exiting.\n"`
  - **Fix**: In `pty.rs` `run_with_password()`:
    - In the `MatchEvent::PasswordPrompt` branch, when `matcher.password_match_count() >= 2`, add: `if verbose { eprintln!("SSHPASS detected prompt, again. Wrong password. Terminating."); }`
    - In the `MatchEvent::HostKeyUnknown` branch, add: `if verbose { eprintln!("SSHPASS detected host authentication prompt. Exiting."); }`
  - Add integration tests for both messages.

  **Must NOT do**:
  - Do not add verbose messages for HostKeyChanged (original doesn't have this since it doesn't detect it).

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: F1-F4
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/pty.rs:176-181` — Wrong password branch where `password_match_count() >= 2`.
  - `src/pty.rs:206-209` — HostKeyUnknown branch.
  - Original sshpass main.c lines 275-277 — `"SSHPASS detected prompt, again. Wrong password. Terminating.\n"`
  - Original sshpass main.c lines 287-289 — `"SSHPASS detected host authentication prompt. Exiting.\n"`

  **Acceptance Criteria**:
  - [ ] Verbose wrong password stderr contains `"SSHPASS detected prompt, again. Wrong password. Terminating."`
  - [ ] Verbose host key unknown stderr contains `"SSHPASS detected host authentication prompt. Exiting."`

  **QA Scenarios**:

  ```
  Scenario: Verbose wrong password message
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration verbose_wrong_password -- --nocapture (new test)
      2. Assert: stderr contains "SSHPASS detected prompt, again. Wrong password. Terminating."
    Expected Result: Wrong password verbose message matches original
    Evidence: .sisyphus/evidence/task-9-verbose-wrong-password.txt

  Scenario: Verbose host key unknown message
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration verbose_host_key_unknown -- --nocapture (new test)
      2. Assert: stderr contains "SSHPASS detected host authentication prompt. Exiting."
    Expected Result: Host key verbose message matches original
    Evidence: .sisyphus/evidence/task-9-verbose-host-key.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): add verbose messages for wrong password and host key`
  - Files: `src/pty.rs`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 10. Use PTY close instead of SIGKILL for child termination on error

  **What to do**:
  - Currently `terminate_child()` in `pty.rs:352-354` calls `child.kill()` which sends SIGKILL.
  - The original sshpass does: `close(masterpt); close(slavept);` which causes ssh to receive SIGHUP when its controlling TTY is closed, a gentler termination.
  - **Fix**: Replace `terminate_child(&mut child)` calls with logic that:
    1. Drops the master PTY writer (closes write end)
    2. Drops the slave PTY fd
    3. Drops the master PTY reader (closes read end)
    4. Then waits for child with `child.wait()`
  - The PTY close will cause ssh to receive SIGHUP and terminate gracefully.
  - If the child doesn't die within a reasonable time, fall back to SIGKILL (safety net).
  - Note: For the HostKeyUnknown and HostKeyChanged cases, the original only closes master/slave when `ret > 0` (positive error from handleoutput). For wrong password (handled in next prompt match), it also closes.

  **Must NOT do**:
  - Do not change the overall flow of `run_with_password()`.
  - Do not introduce deadlocks with the child wait.

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 2
  - **Blocks**: Task 11
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/pty.rs:352-354` — `terminate_child()` function to rewrite.
  - `src/pty.rs:176-214` — All call sites of `terminate_child()` in the match event handlers.
  - Original sshpass main.c lines 155-167 — `close(masterpt); close(slavept);` in the parent's I/O loop when handleoutput returns positive error.

  **Acceptance Criteria**:
  - [ ] Child receives SIGHUP (via PTY close) instead of SIGKILL on error termination
  - [ ] All existing tests still pass (host key, wrong password)
  - [ ] No zombie child processes left behind

  **QA Scenarios**:

  ```
  Scenario: Child terminates gracefully via PTY close
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration wrong_password -- --nocapture
      2. Assert: test passes (exit 5)
      3. Run: cargo test --test integration host_key_unknown -- --nocapture
      4. Assert: test passes (exit 6)
    Expected Result: All error-termination tests still pass
    Evidence: .sisyphus/evidence/task-10-pty-close.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): use PTY close instead of SIGKILL for child termination`
  - Files: `src/pty.rs`
  - Pre-commit: `cargo test`

- [x] 11. Defer stdin/fd password reading to prompt detection time

  **What to do**:
  - Currently, password resolution happens in `main.rs:51` BEFORE PTY creation. For stdin mode, this means the password is read from stdin immediately, blocking until available.
  - The original sshpass reads from stdin/fd **lazily** — `write_pass()` is called only when the password prompt is detected.
  - **Fix approach**: Introduce a `DeferredPassword` enum:
    ```rust
    enum Password {
        Resolved(SecretString),
        Deferred(PasswordResolver),
    }
    ```
  - For `-p`, `-e`, `-k`, and keychain: resolve immediately (they don't read from stdin).
  - For stdin, `-f`, and `-d`: defer resolution. Pass the `Password::Deferred(resolver)` into `run_with_password()`.
  - In `run_with_password()`, when a password prompt is detected and the password is `Deferred`, resolve it at that point.
  - After resolution, replace the `Deferred` with `Resolved`.
  - Note: For `-f` (file) mode, the original also reads lazily. But since files are not shared with the child process, eager reading is fine. Only stdin is truly contended. Still, match the original by deferring all.

  **Must NOT do**:
  - Do not change keychain or 1Password password resolution (always eager).
  - Do not break the existing `run_with_password()` signature unnecessarily — add a new parameter or use the enum approach.

  **Recommended Agent Profile**:
  - **Category**: `deep`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 3 (sequential after Wave 2)
  - **Blocks**: F1-F4
  - **Blocked By**: Task 10

  **References**:

  **Pattern References**:
  - `src/main.rs:51-53` — Current eager `resolve_password()` call.
  - `src/pty.rs:120-131` — `run_with_password()` signature that receives `&SecretString`.
  - `src/password.rs:222-255` — `PasswordResolver` enum that could be extended.
  - Original sshpass main.c `write_pass()` lines 293-316 — Lazy reading for PWT_STDIN, PWT_FD, PWT_FILE modes.

  **Acceptance Criteria**:
  - [ ] `echo pass | sshpassx ssh host` works correctly (stdin not consumed before prompt)
  - [ ] `-f` and `-d` modes still work
  - [ ] `-p` and `-e` modes still work (resolved eagerly)
  - [ ] `-k` / keychain modes still work (resolved eagerly)

  **QA Scenarios**:

  ```
  Scenario: Stdin password read at prompt time
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration stdin_mode_success -- --nocapture
      2. Assert: test passes (exit 0, stdout "Welcome")
    Expected Result: stdin password correctly consumed at prompt match time
    Evidence: .sisyphus/evidence/task-11-stdin-deferred.txt

  Scenario: File password still works
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration file_flag_success -- --nocapture
      2. Assert: test passes
    Expected Result: File reading unaffected
    Evidence: .sisyphus/evidence/task-11-file-still-works.txt

  Scenario: -p still works eagerly
    Tool: Bash (cargo test)
    Steps:
      1. Run: cargo test --test integration password_flag_success -- --nocapture
      2. Assert: test passes
    Expected Result: Argument password unaffected
    Evidence: .sisyphus/evidence/task-11-arg-still-works.txt
  ```

  **Commit**: YES
  - Message: `feat(compat): defer stdin/fd password reading to prompt detection`
  - Files: `src/main.rs`, `src/pty.rs`, `src/password.rs`
  - Pre-commit: `cargo test`

---

## Final Verification Wave

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, run command). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Check evidence files exist in .sisyphus/evidence/. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo clippy` + `cargo test`. Review all changed files for: `as any`/unsafe without justification, empty catches, console.log in prod, commented-out code, unused imports. Check AI slop: excessive comments, over-abstraction, generic names.
  Output: `Build [PASS/FAIL] | Clippy [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [x] F3. **Real QA Run** — `unspecified-high`
  Start from clean state. Execute EVERY QA scenario from EVERY task — follow exact steps, capture evidence. Test cross-task integration. Save to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff (git log/diff). Verify 1:1 — everything in spec was built (no missing), nothing beyond spec was built (no creep). Check "Must NOT do" compliance. Flag unaccounted changes.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

---

## Commit Strategy

Each task = one atomic commit with format: `feat(compat): <description>`

- **T1**: `feat(compat): exit 1 when -e used without SSHPASS env var` — password.rs, cli.rs, error.rs, tests/integration.rs
- **T2**: `feat(compat): show help and exit 0 when no command given` — cli.rs, main.rs, tests/integration.rs
- **T3**: `feat(compat): return 255 for signal-killed child processes` — pty.rs, tests/integration.rs
- **T4**: `feat(compat): pass through child exit code without ParseError mapping` — pty.rs, tests/integration.rs, tests/pty_integration.rs
- **T5**: `feat(compat): add -V version flag` — cli.rs, main.rs, tests/integration.rs
- **T6**: `feat(compat): add signal-exit mode to fake_ssh test helper` — src/bin/fake_ssh.rs
- **T7**: `feat(compat): add SSHPASS read verbose messages` — pty.rs, tests/pty_integration.rs
- **T8**: `feat(compat): match original sshpass verbose message wording` — pty.rs, tests/integration.rs, tests/pty_integration.rs
- **T9**: `feat(compat): add verbose messages for wrong password and host key` — pty.rs, tests/integration.rs
- **T10**: `feat(compat): use PTY close instead of SIGKILL for child termination` — pty.rs
- **T11**: `feat(compat): defer stdin/fd password reading to prompt detection` — main.rs, pty.rs, password.rs

---

## Success Criteria

### Verification Commands
```bash
cargo test 2>&1                    # Expected: all tests pass, 0 failures
cargo clippy 2>&1                  # Expected: no warnings
cargo build --release 2>&1         # Expected: clean build
```

### Final Checklist
- [ ] All "Must Have" behaviors present
- [ ] All "Must NOT Have" guardrails respected
- [ ] All tests pass (existing + new)
- [ ] Exit codes match original sshpass for all shared scenarios
- [ ] Verbose output matches original format

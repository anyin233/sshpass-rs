# Fix SIGWINCH Terminal Resize Causing SSH Disconnect

## TL;DR

> **Quick Summary**: Fix critical bug where resizing the terminal window disconnects SSH with "I/O error: Inappropriate ioctl for device (os error 25)". The root cause is `propagate_winsize()` treating ioctl failures as fatal errors that abort the I/O loop.
> 
> **Deliverables**:
> - `propagate_winsize()` errors become non-fatal (logged, not propagated)
> - `isatty()` guard prevents ioctl on non-terminal fds
> - Unit tests covering all error paths
> 
> **Estimated Effort**: Quick (< 1 hour)
> **Parallel Execution**: NO — sequential (3 small tasks, tightly coupled)
> **Critical Path**: Task 1 (tests) → Task 2 (fix) → Task 3 (verify)

---

## Context

### Original Request
Terminal window resize disconnects SSH session with error: `I/O error: Inappropriate ioctl for device (os error 25)`

### Interview Summary
**Key Discussions**:
- Root cause identified: `propagate_winsize()` in `src/signals.rs` returns `Err(ENOTTY)` which propagates through `check_and_handle()` → I/O loop → session abort
- C sshpass reference: uses `/dev/tty` and silently ignores ioctl failures in SIGWINCH handler
- User initially wanted `portable_pty`'s `resize()` method, but Metis analysis showed it does the exact same ioctl internally — adds `Arc<Mutex>` complexity for zero benefit

**Research Findings**:
- `portable_pty::MasterPty::resize()` internally calls `ioctl(fd, TIOCSWINSZ, ...)` — identical to current code
- Refactoring to use it would require `Arc<Mutex<Box<dyn MasterPty + Send>>>` — disproportionate complexity for a bug fix
- `/dev/tty` may not exist in containers/chroot/nohup — adds new failure mode without fixing the root cause
- The real fix is simple: don't let resize failures kill the session

### Metis Review
**Identified Gaps** (addressed):
- MasterPty refactor descoped: same underlying syscall, adds complexity, zero bug-fix value
- `/dev/tty` fallback descoped: new failure modes, not needed when Fix 1+2 cover the bug
- Both ioctl calls need protection (TIOCGWINSZ AND TIOCSWINSZ), not just the read side
- `propagate_winsize()` should accept `terminal_fd: RawFd` parameter for testability
- Verbose flag needed in SignalHandler for logging suppressed errors

---

## Work Objectives

### Core Objective
Make `propagate_winsize()` errors non-fatal so terminal resize never disconnects the SSH session.

### Concrete Deliverables
- Modified `src/signals.rs`: error-tolerant `propagate_winsize()` with `isatty()` guard
- New unit tests: non-TTY stdin, invalid master_fd, SIGWINCH flag processing
- All existing tests passing

### Definition of Done
- [ ] `cargo test` — all tests pass (including new ones)
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo build` — compiles successfully
- [ ] Resizing terminal during SSH session does NOT disconnect

### Must Have
- `propagate_winsize()` errors are logged (if verbose) but never propagated
- `isatty()` check before `TIOCGWINSZ` on stdin
- Both ioctl calls (read and write side) protected against failure
- Unit tests for error paths

### Must NOT Have (Guardrails)
- MUST NOT change `SignalHandler` to hold `MasterPty` or `Arc<MasterPty>` — keep `RawFd`
- MUST NOT add `/dev/tty` fallback — defer to follow-up
- MUST NOT touch SIGINT/SIGTERM/SIGTSTP/SIGHUP/SIGCHLD handling
- MUST NOT change function signatures beyond what's needed for the fix (except parameterizing terminal_fd for testability)
- MUST NOT add retry logic for failed ioctl calls
- MUST NOT create tests requiring real SSH server or network access
- MUST NOT add log/tracing crate dependencies — use existing `eprintln!` pattern

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES (`cargo test`, unit tests in signals.rs, integration tests)
- **Automated tests**: YES (TDD — red → green → refactor)
- **Framework**: `cargo test` (built-in)
- **TDD**: Task 1 writes failing tests, Task 2 makes them pass

### QA Policy
Every task includes agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Library/Module**: Use Bash (`cargo test`, `cargo clippy`, `cargo build`)

---

## Execution Strategy

### Sequential Execution (3 tasks)

```
Task 1: Write failing tests for propagate_winsize error paths [quick]
  ↓
Task 2: Fix propagate_winsize + check_and_handle to make errors non-fatal [quick]
  ↓
Task 3: Final verification — all tests, clippy, build [quick]
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 | — | 2 |
| 2 | 1 | 3 |
| 3 | 2 | — |

### Agent Dispatch Summary

- **Task 1**: `quick` — write test code only
- **Task 2**: `quick` — modify signals.rs (fix + refactor for testability)
- **Task 3**: `quick` — run verification commands

---

## TODOs

- [x] 1. Write failing tests for SIGWINCH error handling (TDD Red Phase)

  **What to do**:
  - Add new unit tests in `src/signals.rs` `mod tests` block:
    1. `test_propagate_winsize_invalid_master_fd`: Create `SignalHandler` with `master_fd = -1`, set `sigwinch` flag to true, call `check_and_handle()` — must return `Ok(())` (currently fails with error)
    2. `test_propagate_winsize_sigwinch_flag_cleared`: Create `SignalHandler` with invalid master_fd, set `sigwinch` flag, call `check_and_handle()` — verify `sigwinch` flag is cleared after handling
    3. `test_check_and_handle_no_signals_ok`: Create `SignalHandler` with no flags set, call `check_and_handle()` — must return `Ok(())`
  - These tests MUST FAIL at this point (red phase) because `propagate_winsize()` currently propagates errors

  **Must NOT do**:
  - Do not modify any production code yet
  - Do not add tests that require network access or real SSH
  - Do not change existing tests

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Adding test functions to an existing test module — single file, straightforward
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `test-driven-development`: Overkill for adding 3 test functions to an existing module

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential (first task)
  - **Blocks**: Task 2
  - **Blocked By**: None

  **References**:

  **Pattern References** (existing code to follow):
  - `src/signals.rs:126-193` — Existing test module with test patterns: `SignalHandler::new(3, 1234)`, `assert!(result.is_ok())`, AtomicBool store/swap/load patterns

  **API/Type References** (contracts to implement against):
  - `src/signals.rs:69-94` — `check_and_handle()` method signature and behavior
  - `src/signals.rs:42-53` — `SignalHandler::new(master_fd, child_pid)` constructor

  **Test References** (testing patterns to follow):
  - `src/signals.rs:136-146` — `test_signal_handler_creation` — pattern for creating test handlers
  - `src/signals.rs:148-153` — `test_register_all_no_panic` — pattern for testing Result returns
  - `src/signals.rs:155-167` — `test_sigwinch_flag` — pattern for setting/checking AtomicBool flags

  **WHY Each Reference Matters**:
  - The existing tests use `SignalHandler::new(3, 1234)` with fd=3 (arbitrary valid-looking but not actually open fd) and pid=1234. New tests should follow this pattern but use fd=-1 for the invalid case.
  - `check_and_handle()` is the public API being tested — tests should call it and assert `Ok(())` or `Err()`
  - Setting flags via `handler.sigwinch.store(true, Ordering::Relaxed)` simulates signal receipt

  **Acceptance Criteria**:

  **QA Scenarios:**

  ```
  Scenario: New tests compile but fail (TDD red phase)
    Tool: Bash (cargo test)
    Preconditions: No production code changes made
    Steps:
      1. Run `cargo test test_propagate_winsize -- --nocapture 2>&1`
      2. Assert: output contains "test result: FAILED" OR tests panic with ioctl error
      3. Run `cargo test test_check_and_handle_no_signals_ok -- --nocapture 2>&1`
      4. Assert: this test PASSES (no flags set = no ioctl called)
    Expected Result: test_propagate_winsize tests FAIL, test_check_and_handle_no_signals_ok PASSES
    Failure Indicators: All tests pass (means tests don't actually exercise the bug path)
    Evidence: .sisyphus/evidence/task-1-red-phase-test-results.txt

  Scenario: Existing tests still pass
    Tool: Bash (cargo test)
    Preconditions: Only new test functions added, no production code changed
    Steps:
      1. Run `cargo test --bin sshpassx -- --nocapture 2>&1`
      2. Assert: all previously-existing tests pass (test_signal_byte_constants, test_signal_handler_creation, test_register_all_no_panic, test_sigwinch_flag, test_sigchld_received_clears_flag, test_all_flags_independent)
    Expected Result: 6 existing tests PASS
    Failure Indicators: Any existing test fails
    Evidence: .sisyphus/evidence/task-1-existing-tests.txt
  ```

  **Commit**: YES
  - Message: `test(signals): add failing tests for SIGWINCH error handling`
  - Files: `src/signals.rs`
  - Pre-commit: `cargo build --tests`

- [x] 2. Fix propagate_winsize to make SIGWINCH errors non-fatal (TDD Green Phase)

  **What to do**:
  - **Step A — Parameterize `propagate_winsize()` for testability**:
    - Change `propagate_winsize(&self)` to `propagate_winsize(&self, terminal_fd: RawFd)` (private method)
    - In `check_and_handle()`, call `self.propagate_winsize(libc::STDIN_FILENO)` (preserves existing behavior)
    - This allows tests to pass any fd (including -1 or a pipe fd) without touching stdin

  - **Step B — Add `isatty()` guard in `propagate_winsize()`**:
    - Before `TIOCGWINSZ` call, check `unsafe { libc::isatty(terminal_fd) } == 1`
    - If not a terminal, return `Ok(())` immediately (nothing to read, nothing to propagate)

  - **Step C — Make both ioctl failures non-fatal**:
    - If `TIOCGWINSZ` on `terminal_fd` fails: log error if verbose, return `Ok(())`
    - If `TIOCSWINSZ` on `self.master_fd` fails: log error if verbose, return `Ok(())`
    - `propagate_winsize()` should NEVER return `Err()`

  - **Step D — Add verbose logging support**:
    - Add `verbose: bool` field to `SignalHandler` struct
    - Update `SignalHandler::new()` to accept `verbose: bool` parameter
    - In suppressed error paths, use `if self.verbose { eprintln!("SSHPASSX: ..."); }`
    - Update call site in `src/main.rs` `build_signal_handler()` to pass the verbose flag

  - **Step E — Update tests from Task 1**:
    - Update test constructors to pass `verbose: false` (or `verbose: true` to verify logging)
    - Ensure all new tests from Task 1 now PASS
    - Update `SignalHandler::new()` calls in existing tests to include the new `verbose` parameter

  **Must NOT do**:
  - Do not change `SignalHandler` to hold `MasterPty` reference
  - Do not add `/dev/tty` fallback
  - Do not touch SIGINT/SIGTERM/SIGTSTP/SIGHUP/SIGCHLD handling
  - Do not add retry logic
  - Do not add `log` or `tracing` crate dependency

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Focused changes to one file (signals.rs) + minor callsite update in main.rs
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `systematic-debugging`: Not debugging — we know the fix already

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential (after Task 1)
  - **Blocks**: Task 3
  - **Blocked By**: Task 1

  **References**:

  **Pattern References** (existing code to follow):
  - `src/signals.rs:109-122` — Current `propagate_winsize()` implementation (the code being fixed)
  - `src/signals.rs:69-94` — `check_and_handle()` where SIGWINCH triggers `propagate_winsize()` at line 87
  - `src/signals.rs:29-38` — `SignalHandler` struct fields (add `verbose: bool` here)
  - `src/signals.rs:42-53` — `SignalHandler::new()` constructor (add `verbose` param)

  **API/Type References** (contracts to implement against):
  - `libc::isatty(fd: c_int) -> c_int` — returns 1 if fd is a terminal, 0 otherwise
  - `libc::STDIN_FILENO` — constant for stdin fd (0)
  - `libc::TIOCGWINSZ` — ioctl to GET window size
  - `libc::TIOCSWINSZ` — ioctl to SET window size
  - `libc::winsize` — struct with `ws_row`, `ws_col`, `ws_xpixel`, `ws_ypixel`

  **Call Site References** (code that creates SignalHandler):
  - `src/main.rs` — search for `SignalHandler::new` or `build_signal_handler` — this is where `verbose` needs to be plumbed through

  **WHY Each Reference Matters**:
  - `propagate_winsize()` is the exact function being fixed — executor must read it to understand current implementation
  - `check_and_handle()` line 87 shows how `propagate_winsize()` is called — the `?` operator is what makes errors fatal
  - `SignalHandler` struct and constructor show where to add the `verbose` field
  - `main.rs` call site must be updated to pass `verbose` flag from CLI args

  **Acceptance Criteria**:

  **QA Scenarios:**

  ```
  Scenario: All new SIGWINCH tests pass (TDD green phase)
    Tool: Bash (cargo test)
    Preconditions: Task 1 tests exist, Task 2 fixes applied
    Steps:
      1. Run `cargo test test_propagate_winsize -- --nocapture 2>&1`
      2. Assert: output contains "test result: ok" — all propagate_winsize tests PASS
      3. Run `cargo test test_check_and_handle_no_signals_ok -- --nocapture 2>&1`
      4. Assert: test PASSES
    Expected Result: ALL new tests from Task 1 now pass
    Failure Indicators: Any test still fails
    Evidence: .sisyphus/evidence/task-2-green-phase-results.txt

  Scenario: All existing tests still pass (no regression)
    Tool: Bash (cargo test)
    Preconditions: All changes applied
    Steps:
      1. Run `cargo test --bin sshpassx -- --nocapture 2>&1`
      2. Assert: ALL unit tests pass (both existing signal tests and new propagate_winsize tests)
      3. Run `cargo test --test pty_integration 2>&1`
      4. Assert: integration tests pass
      5. Run `cargo test --test integration 2>&1`
      6. Assert: integration tests pass
    Expected Result: Zero test failures across entire test suite
    Failure Indicators: Any test failure
    Evidence: .sisyphus/evidence/task-2-regression-results.txt

  Scenario: Code compiles and passes clippy
    Tool: Bash (cargo clippy, cargo build)
    Preconditions: All changes applied
    Steps:
      1. Run `cargo clippy -- -D warnings 2>&1`
      2. Assert: zero warnings, exit code 0
      3. Run `cargo build 2>&1`
      4. Assert: compilation succeeds, exit code 0
    Expected Result: Clean build with no warnings
    Failure Indicators: Any clippy warning or build failure
    Evidence: .sisyphus/evidence/task-2-build-results.txt
  ```

  **Commit**: YES
  - Message: `fix(signals): make SIGWINCH window resize errors non-fatal`
  - Files: `src/signals.rs`, `src/main.rs`
  - Pre-commit: `cargo test && cargo clippy -- -D warnings`

---

## Final Verification Wave

> Run after ALL implementation tasks. ALL must APPROVE.
> Present consolidated results to user and get explicit "okay" before completing.

- [x] F1. **Plan Compliance Audit** — `oracle`

  ```
  Scenario: All "Must Have" items are implemented
    Tool: Bash (grep) + Read
    Preconditions: All implementation tasks (1-2) committed
    Steps:
      1. Read `src/signals.rs` — find `propagate_winsize` function
      2. Assert: function contains `libc::isatty` guard (isatty check present)
      3. Assert: function does NOT return `Err()` — all error paths return `Ok(())`
      4. Assert: both `TIOCGWINSZ` and `TIOCSWINSZ` ioctl failures are caught (not propagated via `?`)
      5. Run `grep -n "verbose" src/signals.rs` — assert verbose field exists in SignalHandler
      6. Run `cargo test test_propagate_winsize 2>&1` — assert new tests exist and pass
    Expected Result: All 4 "Must Have" items verified present
    Failure Indicators: Any "Must Have" missing from implementation

  Scenario: All "Must NOT Have" guardrails respected
    Tool: Bash (grep)
    Preconditions: All implementation tasks committed
    Steps:
      1. Run `grep -rn "MasterPty" src/signals.rs` — assert: no matches (no MasterPty reference)
      2. Run `grep -rn "Arc<Mutex" src/signals.rs` — assert: no matches
      3. Run `grep -rn "/dev/tty" src/signals.rs` — assert: no matches (no /dev/tty fallback)
      4. Run `grep -rn "retry\|Retry\|RETRY" src/signals.rs` — assert: no retry logic
      5. Run `grep -n "log::\|tracing::" src/signals.rs` — assert: no log/tracing crate usage
      6. Read `Cargo.toml` — assert: no new `log` or `tracing` dependency added
    Expected Result: Zero "Must NOT Have" violations
    Failure Indicators: Any grep match found for forbidden patterns
  ```
  Output: `Must Have [N/4] | Must NOT Have [N/6] | VERDICT: APPROVE/REJECT`

- [x] F2. **Code Quality Review** — `unspecified-high`

  ```
  Scenario: Build, lint, and test all pass
    Tool: Bash (cargo)
    Preconditions: All implementation tasks committed
    Steps:
      1. Run `cargo clippy -- -D warnings 2>&1` — assert: exit code 0, zero warnings
      2. Run `cargo test 2>&1` — assert: exit code 0, all tests pass
      3. Run `cargo build --release 2>&1` — assert: exit code 0, compilation succeeds
    Expected Result: Clean build, zero clippy warnings, all tests pass
    Failure Indicators: Non-zero exit code from any command

  Scenario: No code quality anti-patterns in changed files
    Tool: Read + Bash (grep)
    Preconditions: Implementation complete
    Steps:
      1. Read `src/signals.rs` — scan for `as any`, `@ts-ignore` (N/A for Rust but check anyway)
      2. Run `grep -n "// TODO\|// FIXME\|// HACK\|unwrap()" src/signals.rs` — note any new instances
      3. Run `grep -n "println!\|dbg!" src/signals.rs` — assert: no debug print statements in production code
      4. Verify all new `unsafe` blocks have safety comments explaining why they're sound
    Expected Result: No anti-patterns found
    Failure Indicators: Debug prints, uncommented unsafe, TODO/HACK without justification
  ```
  Output: `Build [PASS/FAIL] | Clippy [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

- [x] F3. **Agent-Executed Functional QA** — `unspecified-high`

  ```
  Scenario: PTY resize does not crash the session (agent-executed, no SSH needed)
    Tool: Bash + interactive_bash (tmux)
    Preconditions: `cargo build --release` succeeds
    Steps:
      1. Run `cargo build --release 2>&1` — assert: exit code 0
      2. Start a tmux session: `new-session -d -s sigwinch-test -x 80 -y 24`
      3. Send command: `send-keys -t sigwinch-test "./target/release/sshpassx -p test /bin/bash -i" Enter`
      4. Wait 2 seconds for bash to start
      5. Send a test command: `send-keys -t sigwinch-test "echo RESIZE_TEST_ALIVE" Enter`
      6. Capture output: `capture-pane -t sigwinch-test -p` — assert: contains "RESIZE_TEST_ALIVE"
      7. Resize the tmux pane to trigger SIGWINCH: `resize-pane -t sigwinch-test -x 120 -y 40`
      8. Wait 1 second
      9. Send another test command: `send-keys -t sigwinch-test "echo AFTER_RESIZE_STILL_ALIVE" Enter`
      10. Capture output: `capture-pane -t sigwinch-test -p` — assert: contains "AFTER_RESIZE_STILL_ALIVE"
      11. Resize again (smaller): `resize-pane -t sigwinch-test -x 60 -y 20`
      12. Wait 1 second
      13. Send final test: `send-keys -t sigwinch-test "echo SECOND_RESIZE_OK" Enter`
      14. Capture output: `capture-pane -t sigwinch-test -p` — assert: contains "SECOND_RESIZE_OK"
      15. Clean up: `send-keys -t sigwinch-test "exit" Enter` then `kill-session -t sigwinch-test`
    Expected Result: Session stays alive through 2 resize events, all echo commands produce output
    Failure Indicators: "I/O error", "Inappropriate ioctl", session terminates, echo output missing
    Evidence: Capture tmux pane output after each step to `.sisyphus/evidence/final-qa/f3-resize-test.txt`

  Scenario: PTY resize with non-TTY stdin (piped input) does not crash
    Tool: Bash
    Preconditions: Binary built
    Steps:
      1. Run `echo "echo PIPED_TEST; exit" | timeout 5 ./target/release/sshpassx -p test /bin/bash 2>&1`
      2. Assert: exit code is 0 or 124 (timeout), NOT 3 (RuntimeError)
      3. Assert: stderr does NOT contain "Inappropriate ioctl"
    Expected Result: No ENOTTY error when stdin is a pipe
    Failure Indicators: Exit code 3, stderr contains "ioctl" error
    Evidence: `.sisyphus/evidence/final-qa/f3-pipe-test.txt`
  ```
  Output: `Resize Test [PASS/FAIL] | Pipe Test [PASS/FAIL] | VERDICT`

- [x] F4. **Scope Fidelity Check** — `deep`

  > Note: `.sisyphus/evidence/` files are QA artifacts, NOT source code modifications. They are excluded from this scope check.

  ```
  Scenario: Only planned files were modified
    Tool: Bash (git diff)
    Preconditions: All implementation tasks committed
    Steps:
      1. Run `git diff --name-only HEAD~2 HEAD` (2 commits = Task 1 + Task 2)
      2. Assert: output contains ONLY `src/signals.rs` and optionally `src/main.rs`
      3. Assert: output does NOT contain any other `src/*.rs` file
      4. Assert: output does NOT contain `Cargo.toml` (no new dependencies added)
    Expected Result: Exactly 1-2 files modified: `src/signals.rs`, `src/main.rs`
    Failure Indicators: Any unexpected file in the diff

  Scenario: No cross-contamination of other signal handlers
    Tool: Bash (git diff) + Read
    Preconditions: All implementation committed
    Steps:
      1. Run `git diff HEAD~2 HEAD -- src/signals.rs`
      2. Read the diff output
      3. Assert: changes are ONLY in `propagate_winsize()`, `check_and_handle()` SIGWINCH branch, `SignalHandler` struct/new(), and `mod tests`
      4. Assert: SIGINT handler code (lines 70-72) is UNCHANGED
      5. Assert: SIGTSTP handler code (lines 74-76) is UNCHANGED
      6. Assert: SIGTERM handler code (lines 78-80) is UNCHANGED
      7. Assert: SIGHUP handler code (lines 82-84) is UNCHANGED
      8. Assert: SIGCHLD handler code (lines 90-91) is UNCHANGED
    Expected Result: Zero changes to non-SIGWINCH signal handlers
    Failure Indicators: Any diff hunk touching SIGINT/SIGTSTP/SIGTERM/SIGHUP/SIGCHLD handling logic
  ```
  Output: `Files Changed [N] | Scope Violations [N] | Cross-Contamination [CLEAN/N issues] | VERDICT`

---

## Commit Strategy

| # | Message | Files | Pre-commit |
|---|---------|-------|------------|
| 1 | `test(signals): add failing tests for SIGWINCH error handling` | `src/signals.rs` | `cargo build --tests` |
| 2 | `fix(signals): make SIGWINCH window resize errors non-fatal` | `src/signals.rs`, `src/main.rs` | `cargo test && cargo clippy -- -D warnings` |

---

## Success Criteria

### Verification Commands
```bash
cargo test                        # Expected: all tests pass
cargo clippy -- -D warnings       # Expected: zero warnings
cargo build --release             # Expected: clean build
```

### Final Checklist
- [ ] All "Must Have" present: non-fatal errors, isatty guard, both ioctl paths protected, unit tests
- [ ] All "Must NOT Have" absent: no MasterPty refactor, no /dev/tty, no retry logic, no log/tracing crate
- [ ] All tests pass
- [ ] Terminal resize no longer disconnects SSH

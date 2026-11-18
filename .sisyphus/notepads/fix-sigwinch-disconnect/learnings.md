# Learnings

## 2026-03-29 Session Start
- Project is a binary-only crate (no lib target) — use `cargo test --bin sshpassx` for unit tests
- Verbose logging uses `eprintln!` with `verbose: bool` flag, no log/tracing crate
- `SignalHandler` uses `signal_hook::flag` (AtomicBool) — signals processed in main loop, not signal context
- `portable_pty::MasterPty::resize()` does identical ioctl internally — no benefit to using it
- Existing tests use `SignalHandler::new(3, 1234)` pattern with arbitrary fd/pid values

## TDD Red Phase — SIGWINCH Tests (2026-03-29)

### Test Results
- `test_propagate_winsize_invalid_master_fd`: FAILS as expected (red phase). `check_and_handle()` propagates `Err(ENOTTY)` via `?` when `master_fd = -1`.
- `test_propagate_winsize_sigwinch_flag_cleared`: PASSES already. The `sigwinch.swap(false, ...)` in `check_and_handle()` clears the flag *before* calling `propagate_winsize()`, so the flag is consumed even when the ioctl fails.
- `test_check_and_handle_no_signals_ok`: PASSES (baseline — no flags set, no ioctl calls made).

### Key Insight
The `sigwinch` flag is cleared atomically via `swap(false, ...)` at line 86 *before* `propagate_winsize()` is called. This means flag consumption is already correct — the bug is only that errors from `propagate_winsize()` are fatal (propagated via `?`). The fix will need to swallow/log the error instead of propagating it.

### Pattern for Invalid FD Tests
Use `SignalHandler::new(-1, 1234)` — fd=-1 causes ioctl to fail with ENOTTY on macOS/Linux.

## Task: fix-sigwinch-disconnect (Green Phase)

### Changes Made
- `src/signals.rs`: Added `verbose: bool` field to `SignalHandler` struct
- `src/signals.rs`: `SignalHandler::new()` now takes `verbose: bool` as 3rd param
- `src/signals.rs`: `propagate_winsize()` now takes `terminal_fd: RawFd` param
- `src/signals.rs`: `propagate_winsize()` has `isatty()` guard — returns `Ok(())` if fd is not a terminal
- `src/signals.rs`: Both ioctl failures are now non-fatal — log if verbose, return `Ok(())`
- `src/signals.rs`: `check_and_handle()` uses `let _ = self.propagate_winsize(libc::STDIN_FILENO)` (no `?`)
- `src/signals.rs`: All 8 test constructors updated to `SignalHandler::new(fd, pid, false)`
- `src/main.rs`: `build_signal_handler()` accepts `verbose: bool`, passes to `SignalHandler::new()`
- `src/main.rs`: Call site passes `cli.verbose`

### Key Patterns
- `isatty()` guard is the primary defense — prevents ioctl on non-terminal fds (e.g., fd=-1 in tests)
- `let _ =` discards Result explicitly when the function is guaranteed non-Err
- Pre-existing test failure `list_standalone_prints_empty_for_fresh_store` is keychain pollution, unrelated

### Verification
- `cargo test --bin sshpassx`: 120 passed, 1 pre-existing failure (keychain pollution)
- `cargo clippy -- -D warnings`: zero warnings
- `cargo build`: clean

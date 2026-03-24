
## Task 5: signals.rs — Signal Handling

### Key Findings
- `nix` v0.29 `unistd::write()` requires `AsFd` trait, not raw `RawFd`. Use `BorrowedFd::borrow_raw()` (unsafe) to wrap `RawFd`.
- `libc` crate is re-exported via `nix::libc` — no need to add `libc` as a direct dependency.
- `signal_hook::flag::register()` returns `io::Result<SigId>` — clean `?` propagation works.
- `TIOCGWINSZ`/`TIOCSWINSZ` ioctl constants available via `nix::libc` (or `libc` crate).
- Signal-hook pattern: `Arc<AtomicBool>` per signal, `swap(false, Relaxed)` in check loop for atomic check-and-clear.
- SIGCHLD handler is intentionally empty — its purpose is just to break the poll/select loop.
- Tests can verify AtomicBool flag behavior without actually sending signals (store/load/swap).

### Patterns Used
- `signal_hook::flag::register(SIGNAL, Arc::clone(&flag))` for safe signal registration
- `flag.swap(false, Ordering::Relaxed)` for atomic check-and-clear in main loop
- `unsafe { BorrowedFd::borrow_raw(raw_fd) }` to bridge RawFd → AsFd for nix v0.29
- `unsafe { libc::ioctl(...) }` for TIOCGWINSZ/TIOCSWINSZ terminal size propagation

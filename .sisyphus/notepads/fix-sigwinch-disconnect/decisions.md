# Decisions

## 2026-03-29 Planning Phase
- DESCOPED: MasterPty refactor — same underlying syscall, adds Arc<Mutex> complexity for zero benefit
- DESCOPED: /dev/tty fallback — new failure modes in containers/chroot/nohup
- INCLUDED: Parameterize terminal_fd for testability (minor signature change)
- INCLUDED: verbose field in SignalHandler for logging suppressed errors

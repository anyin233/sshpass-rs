2026-03-25: SSH alias key derivation can stay additive by keeping `parse_user_at_host` unchanged and falling back to `ssh -G` only for wrapped commands whose basename ends with `ssh`.
2026-03-25: Flag-aware destination parsing needs to skip both flags and their values for SSH options like `-W`, `-F`, `-o`, `-p`, and `-i` so alias detection lands on the real destination argument.

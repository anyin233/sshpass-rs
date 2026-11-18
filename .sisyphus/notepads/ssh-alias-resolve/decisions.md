2026-03-25: Implemented alias resolution through `ssh -G` instead of parsing SSH config directly, preserving support for includes and OpenSSH-specific resolution behavior without new dependencies.
2026-03-25: Propagate `-F <path>` into the `ssh -G` probe so auto-resolution uses the same config file as the wrapped SSH invocation.

# Issues

## 2026-03-28 Environment gotcha

- `cargo test` can fail spuriously when `SSHPASSX_BACKEND` or `SSHPASS_RS_BACKEND` is exported in the shell: standalone keychain tests then hit the real configured backend instead of the temporary file backend used by the tests

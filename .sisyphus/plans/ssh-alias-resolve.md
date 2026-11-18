# SSH Alias Auto-Resolution for Keychain Key Derivation

## TL;DR

> **Quick Summary**: Add fallback `ssh -G` resolution so `-k` mode automatically derives `user@host` from SSH aliases and `-W` invocations, eliminating the need for `--key` in ProxyCommand scenarios.
> 
> **Deliverables**:
> - `find_ssh_destination()` — flag-aware SSH arg parser
> - `parse_ssh_g_output()` — `ssh -G` stdout parser
> - `resolve_via_ssh_config()` — orchestrator calling `ssh -G`
> - Wiring in `main.rs` with verbose logging
> - Unit + integration tests
> 
> **Estimated Effort**: Short
> **Parallel Execution**: NO — sequential (3 TDD commits building on each other)
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Task 5

---

## Context

### Original Request
Auto-resolve SSH aliases to `user@host` keychain keys. When sshpassx encounters `ssh -W %h:%p gw` or `ssh myalias`, resolve the alias via `ssh -G` to find the actual user and hostname from SSH config.

### Interview Summary
**Key Discussions**:
- User hit "unable to derive keychain key" error with `ProxyCommand sshpassx -k ssh -W %h:%p inf-gw`
- Fix was `--key` explicit override, but user wants automatic resolution
- `--key` should still override auto-resolution

**Research Findings**:
- `ssh -G <alias>` outputs resolved config as `key value` pairs (lowercase, one per line)
- Available since OpenSSH 7.0 (2015) — universal on modern systems
- Returns exit 0 even for unknown hosts (uses defaults) — acceptable since keychain lookup fails gracefully
- Fast (~5ms, no network I/O)
- Handles Include, Match, wildcards — all complexity delegated to SSH itself

### Metis Review
**Identified Gaps** (addressed):
- `-F` flag propagation: MUST pass `-F <configfile>` to `ssh -G` when present in SSH args
- Command gating: MUST only attempt resolution when `args[0]` basename ends with `ssh` (not scp/sftp)
- Output parsing: MUST split on first space only (values can contain spaces)

---

## Work Objectives

### Core Objective
When `-k` is used and `parse_user_at_host` fails, automatically resolve SSH aliases via `ssh -G` to construct the keychain key.

### Concrete Deliverables
- 4 new functions in `src/cli.rs`
- 1-line wiring change in `src/main.rs` + verbose logging
- Unit tests for all pure functions
- Integration tests with mock_ssh fixture
- Updated README section

### Definition of Done
- [ ] `cargo test` — all tests pass (existing + new)
- [ ] `sshpassx -k ssh -W %h:%p <alias>` resolves keychain key automatically
- [ ] `sshpassx --key explicit ssh <alias>` still uses explicit key
- [ ] `sshpassx -k ssh user@host` still uses direct parsing (no ssh -G call)

### Must Have
- Fallback chain: `parse_user_at_host` → `resolve_via_ssh_config` → error
- `-F` flag propagated to `ssh -G`
- Only attempt resolution for `ssh` commands (not scp/sftp)
- Verbose logging of resolution attempts

### Must NOT Have (Guardrails)
- DO NOT modify existing `parse_user_at_host` behavior/signature — it stays as-is, new logic is additive
- DO NOT add new dependencies to Cargo.toml
- DO NOT add new error variants to SshpassError — resolution failure returns None
- DO NOT add new CLI flags (no `--no-resolve`)
- DO NOT cache resolved aliases
- DO NOT parse SSH config files directly in Rust — delegate to `ssh -G`
- DO NOT support scp/sftp resolution (different flag semantics)
- DO NOT add excessive comments — code should be self-documenting

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES
- **Automated tests**: YES (TDD)
- **Framework**: cargo test (built-in)

### QA Policy
Every task includes agent-executed QA scenarios.

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Sequential — each builds on previous):
├── Task 1: SSH_FLAGS_WITH_VALUE constant + find_ssh_destination [quick]
├── Task 2: parse_ssh_g_output pure parser [quick]
├── Task 3: resolve_via_ssh_config orchestrator [quick]
├── Task 4: Wire into main.rs + verbose logging [quick]
└── Task 5: Integration tests + mock_ssh fixture [quick]

Wave FINAL:
└── Task 6: Verify full test suite + real SSH test [quick]

Critical Path: T1 → T2 → T3 → T4 → T5 → T6
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1    | —         | 2, 3   |
| 2    | —         | 3      |
| 3    | 1, 2      | 4      |
| 4    | 3         | 5      |
| 5    | 4         | 6      |
| 6    | 5         | —      |

### Agent Dispatch Summary

- All tasks: `quick` category

---

## TODOs

- [x] 1. Add SSH_FLAGS_WITH_VALUE constant and find_ssh_destination function

  **What to do**:
  - Add `SSH_FLAGS_WITH_VALUE: &[&str]` constant in `src/cli.rs` containing all SSH flags that consume a following argument: `["-B", "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p", "-Q", "-R", "-S", "-W", "-w"]`
  - Add `find_ssh_destination(args: &[String]) -> Option<String>` that:
    - Starts at index 1 (skip command name)
    - Skips flags in SSH_FLAGS_WITH_VALUE and their value argument (index += 2)
    - Skips other flags starting with `-` (index += 1)
    - Returns the first positional (non-flag) argument
  - Add `extract_ssh_config_path(args: &[String]) -> Option<String>` that:
    - Scans for `-F` flag and returns its value if present
  - Write unit tests FIRST (TDD):
    - `["ssh", "myalias"]` → `Some("myalias")`
    - `["ssh", "-W", "%h:%p", "gw"]` → `Some("gw")`
    - `["ssh", "-o", "Foo=bar", "-p", "22", "host"]` → `Some("host")`
    - `["ssh", "-p", "2222", "-i", "~/.ssh/id", "-W", "%h:%p", "gw"]` → `Some("gw")`
    - `["ssh"]` → `None`
    - `["ssh", "-v", "-N"]` → `None`
    - `["ssh", "-F", "/custom/config", "myalias"]` → extract_ssh_config_path returns `Some("/custom/config")`

  **Must NOT do**:
  - Do not modify `parse_user_at_host` or `derive_user_at_host_from_args`
  - Do not add comments beyond what's necessary

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Tasks 2, 3
  - **Blocked By**: None

  **References**:
  - `src/cli.rs:320-345` — existing `parse_user_at_host` function for placement and style reference
  - `src/cli.rs:347-493` — existing test module `mod tests` — add new tests here
  - SSH man page flags: `-B`, `-b`, `-c`, `-D`, `-E`, `-e`, `-F`, `-I`, `-i`, `-J`, `-L`, `-l`, `-m`, `-O`, `-o`, `-p`, `-Q`, `-R`, `-S`, `-W`, `-w` all take a value argument

  **Acceptance Criteria**:
  - [ ] `cargo test -- find_ssh_destination` → all tests pass
  - [ ] `cargo test -- extract_ssh_config_path` → all tests pass

  **QA Scenarios:**

  ```
  Scenario: find_ssh_destination skips -W flag and value
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test -- test_find_ssh_destination_with_w_flag --exact`
      2. Assert exit code 0 and "1 passed"
    Expected Result: Test passes, returns "gw" for ["ssh", "-W", "%h:%p", "gw"]

  Scenario: find_ssh_destination returns None for flags-only
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test -- test_find_ssh_destination_no_destination --exact`
      2. Assert exit code 0 and "1 passed"
    Expected Result: Test passes, returns None for ["ssh", "-v", "-N"]
  ```

  **Commit**: YES
  - Message: `feat(cli): add SSH flag-aware destination finder`
  - Files: `src/cli.rs`
  - Pre-commit: `cargo test`

- [x] 2. Add parse_ssh_g_output pure parser

  **What to do**:
  - Add `parse_ssh_g_output(output: &str) -> Option<(String, String)>` in `src/cli.rs` that:
    - Iterates lines of `ssh -G` stdout
    - For lines starting with `"user "` — extract value (everything after first space)
    - For lines starting with `"hostname "` — extract value (everything after first space)
    - Returns `Some((user, hostname))` when both found, `None` otherwise
    - Breaks early when both are found
  - Write unit tests FIRST (TDD):
    - Normal output → `Some(("myuser", "myhost.com"))`
    - Missing user → `None`
    - Missing hostname → `None`
    - Empty string → `None`
    - Real-world multi-line output → correct extraction

  **Must NOT do**:
  - Do not trim/lowercase input — `ssh -G` output is already lowercase
  - Do not parse any fields other than `user` and `hostname`

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 3
  - **Blocked By**: Task 1 (same file)

  **References**:
  - `ssh -G` output format: `key value\n` pairs, lowercase, split on first space
  - Example: `user yanweiye\nhostname 129.215.90.75\nport 22\naddressfamily any\n...`

  **Acceptance Criteria**:
  - [ ] `cargo test -- parse_ssh_g_output` → all tests pass

  **QA Scenarios:**

  ```
  Scenario: parse_ssh_g_output extracts user and hostname
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test -- test_parse_ssh_g_output_normal --exact`
      2. Assert exit code 0
    Expected Result: Returns Some(("testuser", "10.0.0.1")) for typical ssh -G output

  Scenario: parse_ssh_g_output returns None for incomplete output
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test -- test_parse_ssh_g_output_missing_user --exact`
      2. Assert exit code 0
    Expected Result: Returns None when "user" line is absent
  ```

  **Commit**: YES
  - Message: `feat(cli): add ssh -G output parser`
  - Files: `src/cli.rs`
  - Pre-commit: `cargo test`

- [x] 3. Add resolve_via_ssh_config orchestrator and wire into main.rs

  **What to do**:
  - Add `resolve_via_ssh_config(args: &[String], verbose: bool) -> Option<String>` in `src/cli.rs` that:
    - Checks `args[0]` basename ends with `"ssh"` — returns `None` otherwise
    - Calls `find_ssh_destination(args)` — returns `None` if no destination
    - Builds `ssh -G <destination>` command, adding `-F <path>` if `extract_ssh_config_path(args)` returns a value
    - Runs the command via `std::process::Command`
    - Checks exit code == 0
    - Calls `parse_ssh_g_output` on stdout
    - Returns `Some("user@hostname")` or `None`
    - Verbose logging: `"SSHPASSX: resolving SSH alias '{}' via ssh -G"` and `"SSHPASSX: resolved alias '{}' to keychain key '{}'"` or `"SSHPASSX: failed to resolve SSH alias '{}'"`
  - Make `resolve_via_ssh_config` a `pub` function
  - In `src/main.rs:228-237`, change the `-k` auto-detection block:
    - FROM: `let key = cli::parse_user_at_host(&cli.command).ok_or_else(|| ...)?;`
    - TO: `let key = cli::parse_user_at_host(&cli.command).or_else(|| cli::resolve_via_ssh_config(&cli.command, verbose)).ok_or_else(|| ...)?;`

  **Must NOT do**:
  - Do not modify `parse_user_at_host` signature or behavior
  - Do not add new error variants
  - Do not attempt resolution for non-ssh commands

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 4
  - **Blocked By**: Tasks 1, 2

  **References**:
  - `src/main.rs:228-237` — the `-k` auto-detection block to modify
  - `src/onepassword.rs:153-180` — `run_op` method shows the established `std::process::Command` pattern
  - `src/main.rs:223-227` — `--key` override block (remains unchanged, checked before `-k`)

  **Acceptance Criteria**:
  - [ ] `cargo test` → all tests pass (no regressions)
  - [ ] The `.or_else()` chain is wired in main.rs

  **QA Scenarios:**

  ```
  Scenario: resolve_via_ssh_config rejects non-ssh commands
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test -- test_resolve_rejects_non_ssh --exact`
      2. Assert exit code 0
    Expected Result: Returns None for args starting with "scp"

  Scenario: Full suite passes with no regressions
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test 2>&1`
      2. Assert all test results show "0 failed"
    Expected Result: All existing + new tests pass
  ```

  **Commit**: YES
  - Message: `feat: auto-resolve SSH aliases for keychain key via ssh -G`
  - Files: `src/cli.rs`, `src/main.rs`
  - Pre-commit: `cargo test`

- [x] 4. Add integration tests with mock_ssh fixture

  **What to do**:
  - Create `tests/fixtures/mock_ssh.sh` that mimics `ssh -G`:
    - When args contain `-G`, parse the destination argument
    - Return canned `user` and `hostname` output based on destination
    - `myalias` → `user testuser\nhostname 10.0.0.1\nport 22\n`
    - `gw` → `user admin\nhostname gateway.local\nport 22\n`
    - Unknown → exit 255
    - Make it executable (`chmod +x`)
  - Add integration tests in `tests/integration.rs`:
    - Test alias resolution: `sshpassx -k ssh myalias` with mock → derives key `testuser@10.0.0.1`, use env `SSHPASSX_TEST_KEYCHAIN_FILE` + pre-stored password for that key
    - Test `-W` resolution: `sshpassx -k ssh -W %h:%p gw` with mock → derives key `admin@gateway.local`
    - Test `--key` override: `sshpassx --key explicit ssh myalias` → uses `explicit` key, mock_ssh not consulted
    - Test direct `user@host` bypass: `sshpassx -k ssh user@host` → uses direct parsing, no ssh -G call
    - Use PATH injection to put mock_ssh's directory first, so `ssh` resolves to mock
    - BUT: mock_ssh must also handle normal `ssh` invocations (the actual ssh command that sshpassx wraps) — use `fake_ssh` for the wrapped command instead, or structure tests to only test key derivation
  - Actually, the simplest approach: test the key derivation via `--store` + `-k --list`-style verification, or test that the verbose output shows the resolved key. Since the integration tests already use `SSHPASSX_TEST_KEYCHAIN_FILE`, pre-store a password under the resolved key and verify the flow works.

  **Must NOT do**:
  - Do not require real SSH connections
  - Do not break existing integration tests

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 5
  - **Blocked By**: Task 3

  **References**:
  - `tests/fixtures/mock_op.sh` — established mock script pattern (arg parsing, canned responses)
  - `tests/onepassword_integration.rs` — integration test pattern with mock scripts via PATH injection
  - `tests/integration.rs` — existing integration tests with `fake_ssh` + `SSHPASSX_TEST_KEYCHAIN_FILE`

  **Acceptance Criteria**:
  - [ ] `cargo test --test integration` → all tests pass including new ones
  - [ ] mock_ssh.sh exists and is executable

  **QA Scenarios:**

  ```
  Scenario: Integration test for alias resolution
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test --test integration -- test_alias_resolution`
      2. Assert exit code 0
    Expected Result: Test passes — sshpassx resolves alias via mock ssh -G

  Scenario: Full integration suite has no regressions
    Tool: Bash (cargo test)
    Steps:
      1. Run `cargo test --test integration 2>&1`
      2. Check "0 failed" in output
    Expected Result: All 29+ existing tests + new tests pass
  ```

  **Commit**: YES
  - Message: `test: add integration tests for SSH alias resolution with mock_ssh`
  - Files: `tests/fixtures/mock_ssh.sh`, `tests/integration.rs`
  - Pre-commit: `cargo test`

- [x] 5. Update README and bump version

  **What to do**:
  - Update `README.md` SSH Config Integration section:
    - Note that `--key` is no longer required for alias resolution
    - Explain the automatic fallback: direct parsing → `ssh -G` resolution
    - Keep the `--key` override documented as an explicit escape hatch
  - Update the "Key Auto-Detection" section to describe alias resolution
  - Bump version in `Cargo.toml`: `0.2.0` → `0.3.0`

  **Must NOT do**:
  - Do not use real usernames/hostnames in examples
  - Do not remove the `--key` documentation

  **Recommended Agent Profile**:
  - **Category**: `quick`
  - **Skills**: []

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Sequential
  - **Blocks**: Task 6
  - **Blocked By**: Task 4

  **References**:
  - `README.md:133-140` — "Key Auto-Detection" section
  - `README.md:199-230` — "SSH Config Integration" section
  - `Cargo.toml:3` — version field

  **Acceptance Criteria**:
  - [ ] README documents alias resolution
  - [ ] Version is 0.3.0

  **QA Scenarios:**

  ```
  Scenario: README contains alias resolution docs
    Tool: Bash (grep)
    Steps:
      1. grep "ssh -G" README.md
      2. Assert match found
    Expected Result: README mentions ssh -G resolution
  ```

  **Commit**: YES
  - Message: `docs: document SSH alias auto-resolution and bump to v0.3.0`
  - Files: `README.md`, `Cargo.toml`
  - Pre-commit: `cargo test`

---

## Final Verification Wave

> After ALL tasks — run full test suite and real SSH test.

- [x] F1. **Full test suite** — `cargo test` → all pass, 0 failures
- [x] F2. **Real SSH test** — alias resolution verified: `inf-gw` → `s2774883@ssh.inf.ed.ac.uk` (server timeout is network, not sshpassx)
- [x] F3. **Publish** — `cargo publish` to crates.io as v0.3.0
- [x] F4. **Push** — `git push origin master`

---

## Commit Strategy

| # | Message | Files |
|---|---------|-------|
| 1 | `feat(cli): add SSH flag-aware destination finder` | `src/cli.rs` |
| 2 | `feat(cli): add ssh -G output parser` | `src/cli.rs` |
| 3 | `feat: auto-resolve SSH aliases for keychain key via ssh -G` | `src/cli.rs`, `src/main.rs` |
| 4 | `test: add integration tests for SSH alias resolution with mock_ssh` | `tests/fixtures/mock_ssh.sh`, `tests/integration.rs` |
| 5 | `docs: document SSH alias auto-resolution and bump to v0.3.0` | `README.md`, `Cargo.toml` |

---

## Success Criteria

### Verification Commands
```bash
cargo test                    # Expected: all pass, 0 failures
cargo clippy -- -D warnings   # Expected: no warnings
ssh -o ConnectTimeout=10 ed-jazz1 echo "connection_ok"  # Expected: connection_ok
```

### Final Checklist
- [ ] `sshpassx -k ssh -W %h:%p <alias>` auto-resolves keychain key
- [ ] `sshpassx --key explicit ssh <alias>` still uses explicit key
- [ ] `sshpassx -k ssh user@host` still uses direct parsing
- [ ] `-F` flag propagated to `ssh -G`
- [ ] Non-ssh commands (scp/sftp) do not trigger `ssh -G`
- [ ] All existing tests pass (no regressions)
- [ ] Published to crates.io as v0.3.0

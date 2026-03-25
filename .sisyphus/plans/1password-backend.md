# 1Password Backend for sshpass-rs

## TL;DR

> **Quick Summary**: Add 1Password as an alternative password backend to sshpass-rs by implementing a new `OnePasswordBackend` that shells out to the `op` CLI. Users select the backend via `SSHPASS_RS_BACKEND=op` env var; vault via `SSHPASS_RS_VAULT`.
> 
> **Deliverables**:
> - New `src/onepassword.rs` module with `OnePasswordBackend` implementing `KeychainBackend`
> - Modified `build_keychain_backend()` with env-var-based backend selection
> - `serde`/`serde_json` dependency for JSON parsing
> - Unit tests for JSON parsing, backend operations, and backend selection
> - Integration tests with a mock `op` binary
> - Updated README documenting 1Password usage
> 
> **Estimated Effort**: Medium
> **Parallel Execution**: YES - 3 waves
> **Critical Path**: Task 1 → Task 2 → Task 3 → Task 4 → Task 5 → Task 7

---

## Context

### Original Request
用户希望为 sshpass-rs 添加 1Password 集成，使其可以通过 1Password API 存取 SSH 密码。

### Interview Summary
**Key Discussions**:
- **Integration method**: 1Password CLI (`op`) via subprocess — simplest, no Rust SDK exists, handles auth natively
- **Operations scope**: Full CRUD (store/get/delete/list) matching `KeychainBackend` trait
- **CLI design**: Keep `-k` flag unchanged, use `SSHPASS_RS_BACKEND=op` env var for backend selection
- **Vault**: `SSHPASS_RS_VAULT` env var, defaults to 1Password's default vault if unset
- **Item structure**: Password category, title = key name, password field = secret value, tag = `sshpass-rs`
- **List filtering**: Only items tagged `sshpass-rs` appear in `--list`
- **Test strategy**: Tests-after with mocked `op` CLI output

**Research Findings**:
- No official 1Password Rust SDK; `op` CLI subprocess is the recommended Rust approach
- `op` supports `--format json` on all item operations, enabling reliable machine parsing
- `op item get` uses fuzzy title matching — risk of wrong item retrieval
- `op item create` accepts `--category Password --tags sshpass-rs` for structured creation
- Service account tokens via `OP_SERVICE_ACCOUNT_TOKEN` for automation; biometric for interactive

### Metis Review
**Identified Gaps** (all addressed):
- **Password leaking via CLI args**: RESOLVED — use stdin JSON template piping to `op item create -` (trailing `-` reads template from stdin), never pass passwords as command-line arguments
- **`op item get` fuzzy match risk**: RESOLVED — use `op item list --tags sshpass-rs` + client-side exact title match instead of `op item get` by title
- **`op` CLI not found error**: RESOLVED — catch `io::ErrorKind::NotFound` and produce helpful error message with install URL
- **Error message format compatibility**: RESOLVED — `get()` must produce `"key not found: {key}"` to match `RealKeychainBackend` convention
- **Module organization**: RESOLVED — new `src/onepassword.rs` file, not added to bloated `keychain.rs`
- **`build_keychain_backend()` dual-call pattern**: RESOLVED — keep constructor cheap, validate `op` lazily on first operation
- **Mock testability**: RESOLVED — inject `op_path: String` field in backend struct, defaulting to `"op"`

---

## Work Objectives

### Core Objective
Add 1Password as an alternative password backend to sshpass-rs, accessible via `SSHPASS_RS_BACKEND=op` environment variable, implementing full CRUD through the `op` CLI.

### Concrete Deliverables
- `src/onepassword.rs` — new module with `OnePasswordBackend`, serde structs, parsing functions, unit tests
- Modified `src/main.rs` — `build_keychain_backend()` reads `SSHPASS_RS_BACKEND` env var
- Modified `Cargo.toml` — `serde` + `serde_json` dependencies
- Integration tests with mock `op` binary
- Updated `README.md` with 1Password usage documentation

### Definition of Done
- [ ] `cargo check` compiles without errors
- [ ] `cargo test` — all tests pass (existing + new)
- [ ] `cargo clippy -- -D warnings` — no warnings
- [ ] `cargo fmt --check` — formatting correct
- [ ] `SSHPASS_RS_BACKEND=op` selects 1Password backend
- [ ] Unset `SSHPASS_RS_BACKEND` preserves all existing Keychain behavior
- [ ] Passwords never appear in `op` command-line arguments

### Must Have
- `OnePasswordBackend` implementing all 4 `KeychainBackend` trait methods
- Stdin JSON piping for `op item create` (password security)
- Tag-filtered listing with exact title matching for `get()` (reliability)
- Clear error message when `op` CLI is not installed
- Error message format `"key not found: {key}"` matching existing convention
- `SSHPASS_RS_BACKEND` env var for backend selection (accepts `op` or `1password`)
- `SSHPASS_RS_VAULT` env var for vault selection (optional)
- Injectable `op` binary path for test mockability

### Must NOT Have (Guardrails)
- ❌ Passwords as `op` command-line arguments — security violation
- ❌ `op item get` by title (fuzzy match risk) — use list+filter instead
- ❌ Modifications to `RealKeychainBackend`, `InMemoryKeychainBackend`, or `FileKeychainBackend`
- ❌ Changes to `KeychainBackend` trait signature
- ❌ Changes to `src/pty.rs`, `src/matcher.rs`, `src/signals.rs`
- ❌ Multi-vault per-key selection
- ❌ `op` CLI v1 compatibility
- ❌ Config file for backend selection — env vars only
- ❌ Subprocess timeout handling — `op` manages its own timeouts
- ❌ Cargo feature flags for this backend
- ❌ Retry logic, caching, or session management
- ❌ `op` CLI installation/update management
- ❌ Vault creation/management

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES — `cargo test`, `assert_cmd`, `tempfile`, `predicates`
- **Automated tests**: YES (tests-after)
- **Framework**: `cargo test` (existing)

### QA Policy
Every task MUST include agent-executed QA scenarios.
Evidence saved to `.sisyphus/evidence/task-{N}-{scenario-slug}.{ext}`.

- **Module/Library**: Use Bash (`cargo test`) — run tests, verify output
- **CLI integration**: Use Bash — run `sshpass-rs` with env vars and mock `op`, verify exit codes and output

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — foundation):
├── Task 1: Add serde/serde_json dependencies [quick]
├── Task 2: Create op JSON response types + parsing functions [unspecified-high]
└── Task 3: Create mock op shell script for tests [quick]

Wave 2 (After Wave 1 — core implementation):
├── Task 4: Implement OnePasswordBackend with KeychainBackend trait [deep]
├── Task 4b: Fix KeychainPassword::resolve() error handling for backend failures [unspecified-high]
└── Task 5: Wire backend selection in build_keychain_backend() [unspecified-high]

Wave 3 (After Wave 2 — tests + docs):
├── Task 6: Add unit tests for OnePasswordBackend [unspecified-high]
├── Task 7: Add integration tests with mock op binary [unspecified-high]
└── Task 8: Update README with 1Password documentation [writing]

Wave FINAL (After ALL tasks):
├── Task F1: Plan compliance audit [oracle]
├── Task F2: Code quality review [unspecified-high]
├── Task F3: Real manual QA [unspecified-high]
└── Task F4: Scope fidelity check [deep]
-> Present results -> Get explicit user okay
```

### Dependency Matrix

| Task | Depends On | Blocks |
|------|-----------|--------|
| 1 | — | 2, 4, 5, 6 |
| 2 | 1 | 4, 6 |
| 3 | — | 7 |
| 4 | 1, 2 | 4b, 5, 6, 7 |
| 4b | 4 | 5, 7 |
| 5 | 4, 4b | 7 |
| 6 | 4 | 7 |
| 7 | 3, 5, 6 | — |
| 8 | — | — |

Critical Path: Task 1 → Task 2 → Task 4 → Task 4b → Task 5 → Task 7 → F1-F4 → user okay

**Pre-work step**: Before starting Task 1, record the current HEAD commit hash: `git rev-parse HEAD > .sisyphus/evidence/base-commit.txt`. This is used by F1/F4 verification to diff only this work's changes.

### Agent Dispatch Summary

- **Wave 1**: 3 tasks — T1 → `quick`, T2 → `unspecified-high`, T3 → `quick`
- **Wave 2**: 3 tasks — T4 → `deep`, T4b → `unspecified-high`, T5 → `unspecified-high`
- **Wave 3**: 3 tasks — T6 → `unspecified-high`, T7 → `unspecified-high`, T8 → `writing`
- **FINAL**: 4 tasks — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

> Implementation + Test = ONE Task. Never separate.
> EVERY task MUST have: Recommended Agent Profile + Parallelization info + QA Scenarios.

- [x] 1. Add serde and serde_json dependencies to Cargo.toml

  **What to do**:
  - Add `serde = { version = "1", features = ["derive"] }` and `serde_json = "1"` to `[dependencies]` in `Cargo.toml`
  - Run `cargo check` to verify the project compiles with the new dependencies

  **Must NOT do**:
  - Do not add feature flags or conditional compilation for these deps
  - Do not modify any other dependencies

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single file edit, trivial change
  - **Skills**: []
    - No special skills needed
  - **Skills Evaluated but Omitted**:
    - `uv`: Python-specific, not relevant for Rust/Cargo

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 2, 3)
  - **Blocks**: Tasks 2, 4, 5, 6
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References**:
  - `Cargo.toml:14-28` — Existing dependency format with version specifiers and features

  **API/Type References**:
  - None

  **Test References**:
  - None

  **External References**:
  - `serde` crate: https://docs.rs/serde/latest/serde/
  - `serde_json` crate: https://docs.rs/serde_json/latest/serde_json/

  **WHY Each Reference Matters**:
  - `Cargo.toml:14-28` — Follow the existing formatting style (version strings, feature arrays)

  **Acceptance Criteria**:
  - [ ] `serde` and `serde_json` appear in `[dependencies]` section
  - [ ] `cargo check` compiles without errors

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Dependencies compile successfully
    Tool: Bash (cargo)
    Preconditions: Working Rust toolchain
    Steps:
      1. Run `cargo check` in project root
      2. Assert exit code is 0
      3. Assert stderr does not contain "error"
    Expected Result: Clean compilation with new deps
    Failure Indicators: Compilation errors, version conflicts
    Evidence: .sisyphus/evidence/task-1-deps-compile.txt
  ```

  **Commit**: YES
  - Message: `feat(deps): add serde and serde_json for 1Password JSON parsing`
  - Files: `Cargo.toml`
  - Pre-commit: `cargo check`

- [x] 2. Create op CLI JSON response types and parsing functions

  **What to do**:
  - Create `src/onepassword.rs` as a new module
  - Define serde structs for `op` CLI JSON output (minimal, only fields we use):
    - `OpItem` — `id: String`, `title: String`, `category: String` (all `#[serde(default)]`)
    - `OpItemDetail` — same fields plus `fields: Vec<OpField>`
    - `OpField` — `id: String`, `type_: String` (rename from `type`), `value: Option<String>`, `label: Option<String>` (all `#[serde(default)]`)
  - Implement parsing functions:
    - `fn parse_item_list(json: &str) -> Result<Vec<OpItem>, SshpassError>` — parses `op item list --format json` output, returns Vec of OpItem (id + title). This is used by `get()` and `delete()` which need the ID.
    - `fn parse_item_titles(json: &str) -> Result<Vec<String>, SshpassError>` — convenience wrapper that calls `parse_item_list()` and maps to titles only. Used by `list()`.
    - `fn parse_item_password(json: &str) -> Result<SecretString, SshpassError>` — parses `op item get --format json` output, extracts password field value (looks for field with `id == "password"` or `type == "CONCEALED"`)
  - Add `mod onepassword;` to `src/main.rs`
  - Write unit tests for parsing functions

  **Must NOT do**:
  - Do not implement `KeychainBackend` trait yet (that's Task 4)
  - Do not add any subprocess spawning logic
  - Do not parse fields we don't need (no fingerprints, notes, etc.)

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: New module creation with serde structs and parsing logic requires careful design
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `test-driven-development`: Strategy is tests-after per user preference

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 3)
  - **Parallel Group**: Wave 1 (with Tasks 1, 3) — but depends on Task 1 for serde
  - **Blocks**: Tasks 4, 6
  - **Blocked By**: Task 1

  **References**:

  **Pattern References**:
  - `src/keychain.rs:149-288` — `FileKeychainBackend` hand-rolled JSON parsing — shows current approach; we're replacing this pattern with serde
  - `src/error.rs:38-54` — `SshpassError` enum — use `SshpassError::KeychainAccess(String)` for parse errors

  **API/Type References**:
  - `src/keychain.rs:11-16` — `KeychainBackend` trait — the `list()` returns `Vec<String>` (just titles), the `get()` returns `SecretString`

  **Test References**:
  - `src/keychain.rs:392-468` — Unit test structure for backend operations (assertion patterns)

  **External References**:
  - 1Password CLI `op item list` JSON format: `[{"id":"...", "title":"...", "category":"PASSWORD"}]`
  - 1Password CLI `op item get` JSON format: `{"id":"...", "title":"...", "fields":[{"id":"password", "type":"CONCEALED", "value":"secret"}]}`

  **WHY Each Reference Matters**:
  - `src/keychain.rs:149-288` — Shows what manual JSON parsing looks like; our serde approach should be cleaner
  - `src/error.rs:38-54` — Must use `SshpassError::KeychainAccess` for all errors from this module
  - `src/keychain.rs:11-16` — Return types must match trait expectations (Vec<String> for list, SecretString for get)

  **Acceptance Criteria**:
  - [ ] `src/onepassword.rs` exists with serde structs and parsing functions
  - [ ] `mod onepassword;` added to `src/main.rs`
  - [ ] `cargo test -- onepassword` passes all parsing tests

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Parse valid item list JSON
    Tool: Bash (cargo test)
    Preconditions: Task 1 complete (serde available)
    Steps:
      1. Run `cargo test -- onepassword::tests::test_parse_item_list`
      2. Assert test passes — input: `[{"id":"abc","title":"user@host","category":"PASSWORD"},{"id":"def","title":"root@server","category":"PASSWORD"}]`
      3. Assert output: `vec!["user@host", "root@server"]`
    Expected Result: Titles extracted correctly from JSON array
    Failure Indicators: Parse error, wrong titles, wrong order
    Evidence: .sisyphus/evidence/task-2-parse-list.txt

  Scenario: Parse empty item list JSON
    Tool: Bash (cargo test)
    Preconditions: Task 1 complete
    Steps:
      1. Run `cargo test -- onepassword::tests::test_parse_empty_list`
      2. Assert test passes — input: `[]`
      3. Assert output: `vec![]`
    Expected Result: Empty vec returned for empty JSON array
    Failure Indicators: Parse error on empty array
    Evidence: .sisyphus/evidence/task-2-parse-empty.txt

  Scenario: Parse item detail to extract password
    Tool: Bash (cargo test)
    Preconditions: Task 1 complete
    Steps:
      1. Run `cargo test -- onepassword::tests::test_parse_item_password`
      2. Assert test passes — input JSON with field `{"id":"password","type":"CONCEALED","value":"s3cret"}`
      3. Assert output: SecretString exposing "s3cret"
    Expected Result: Password field correctly extracted
    Failure Indicators: Wrong field selected, empty password
    Evidence: .sisyphus/evidence/task-2-parse-password.txt

  Scenario: Parse JSON with unknown extra fields (forward compat)
    Tool: Bash (cargo test)
    Preconditions: Task 1 complete
    Steps:
      1. Run `cargo test -- onepassword::tests::test_parse_unknown_fields`
      2. Assert test passes — input JSON with extra fields like "version", "urls", "created_at"
      3. Assert parsing still succeeds
    Expected Result: Unknown fields silently ignored
    Failure Indicators: Deserialization error on unknown fields
    Evidence: .sisyphus/evidence/task-2-parse-compat.txt
  ```

  **Commit**: YES
  - Message: `feat(1password): add op CLI JSON response types and parsing`
  - Files: `src/onepassword.rs`, `src/main.rs`
  - Pre-commit: `cargo test`

- [x] 3. Create mock op shell script for integration tests

  **What to do**:
  - Create `tests/fixtures/mock_op.sh` — a shell script that mimics `op` CLI behavior:
    - Parses arguments to determine which command is being called (`item list`, `item get`, `item create`, `item delete`)
    - Returns canned JSON responses for known inputs
    - Returns appropriate exit codes (0 for success, 1 for errors)
    - Writes to stderr for error cases
    - Supports `--format json`, `--vault`, `--tags`, `--category` flags
  - Make script executable (`chmod +x`)
  - The script should handle these scenarios:
    - `op item list --tags sshpass-rs --format json` → returns JSON array of items
    - `op item list --tags sshpass-rs --format json` (empty) → returns `[]`
    - `op item get <id> --format json` → returns item detail JSON with password field
    - `op item create - --format json` (with stdin JSON template) → reads JSON from stdin, returns created item JSON
    - `op item delete <title-or-id>` → success (exit 0) or not-found (exit 1 + stderr)
    - Unknown command → exit 1 with stderr error

  **Must NOT do**:
  - Do not create a Rust binary for the mock (shell script is simpler for test fixtures)
  - Do not mock authentication or service account flows
  - Do not handle real `op` edge cases (rate limiting, network errors)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Shell script creation, no complex logic
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `playwright`: Not a browser task

  **Parallelization**:
  - **Can Run In Parallel**: YES
  - **Parallel Group**: Wave 1 (with Tasks 1, 2)
  - **Blocks**: Task 7
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References**:
  - `src/bin/fake_ssh.rs` — Existing mock binary pattern in the project (Rust-based, but shows mock approach)

  **API/Type References**:
  - None

  **Test References**:
  - `tests/integration.rs:216-233` — How integration tests use env vars to configure test backends

  **External References**:
  - `op item list --format json` output: `[{"id":"...","title":"...","category":"PASSWORD","vault":{"id":"...","name":"..."}}]`
  - `op item get <id> --format json` output: full item with fields array

  **WHY Each Reference Matters**:
  - `src/bin/fake_ssh.rs` — Shows the project's existing mock binary pattern. We use a shell script instead for simplicity but should match the testing philosophy.
  - `tests/integration.rs:216-233` — Shows how env vars inject test configuration (we'll inject mock `op` path similarly)

  **Acceptance Criteria**:
  - [ ] `tests/fixtures/mock_op.sh` exists and is executable
  - [ ] Running `tests/fixtures/mock_op.sh item list --tags sshpass-rs --format json` outputs valid JSON
  - [ ] Running `tests/fixtures/mock_op.sh item get <id> --format json` outputs item detail JSON
  - [ ] Exit codes: 0 for success, 1 for errors

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Mock op list returns valid JSON
    Tool: Bash
    Preconditions: Script created and executable
    Steps:
      1. Run `bash tests/fixtures/mock_op.sh item list --tags sshpass-rs --format json`
      2. Pipe output to `python3 -m json.tool` to validate JSON
      3. Assert exit code 0
      4. Assert output contains at least one item with "title" field
    Expected Result: Valid JSON array of items
    Failure Indicators: Non-JSON output, non-zero exit code
    Evidence: .sisyphus/evidence/task-3-mock-list.txt

  Scenario: Mock op handles unknown command with error
    Tool: Bash
    Preconditions: Script created and executable
    Steps:
      1. Run `bash tests/fixtures/mock_op.sh unknown-command 2>stderr.txt`
      2. Assert exit code 1
      3. Assert stderr.txt contains error message
    Expected Result: Non-zero exit code with stderr error
    Failure Indicators: Exit code 0 for unknown command
    Evidence: .sisyphus/evidence/task-3-mock-error.txt
  ```

  **Commit**: NO (groups with Task 7)

- [x] 4. Implement OnePasswordBackend with KeychainBackend trait

  **What to do**:
  - In `src/onepassword.rs`, add `OnePasswordBackend` struct:
    ```
    pub struct OnePasswordBackend {
        vault: Option<String>,    // from SSHPASS_RS_VAULT
        op_path: String,          // default "op", injectable for tests
    }
    ```
  - Implement constructor: `pub fn new(vault: Option<String>) -> Self` (uses `"op"` default) and `pub fn with_op_path(vault: Option<String>, op_path: String) -> Self` for test injection
  - Implement `KeychainBackend` trait:
    - **`store(key, password)`**: Build JSON template `{"title":"<key>","category":"PASSWORD","tags":["sshpass-rs"],"fields":[{"id":"password","type":"CONCEALED","value":"<password>"}]}`, pipe via stdin to `op item create --format json [--vault V] -` (the trailing `-` tells `op` to read the item template from stdin). Password MUST go through stdin, NEVER as CLI arg.
    - **`get(key)`**: Run `op item list --tags sshpass-rs --format json [--vault V]`, parse response with `parse_item_list()` logic but also get IDs, find exact title match, then run `op item get <id> --format json` and extract password. Return `SshpassError::KeychainAccess("key not found: {key}")` if no match.
    - **`delete(key)`**: Run `op item list --tags sshpass-rs --format json [--vault V]`, find exact title match to get item ID, then run `op item delete <id> [--vault V]`. Return `SshpassError::KeychainAccess("key not found: {key}")` if no match.
    - **`list()`**: Run `op item list --tags sshpass-rs --format json [--vault V]`, parse with `parse_item_list()`, return titles.
  - Helper function: `fn run_op(&self, args: &[&str]) -> Result<String, SshpassError>` — spawns `op` subprocess, captures stdout/stderr, checks exit code. On `io::ErrorKind::NotFound`, return `SshpassError::KeychainAccess("1Password CLI (op) not found. Install from https://1password.com/downloads/command-line/")`. On non-zero exit, include stderr in error.
  - Helper function: `fn run_op_with_stdin(&self, args: &[&str], stdin_data: &str) -> Result<String, SshpassError>` — same but pipes stdin_data to subprocess.
  - Make `OnePasswordBackend` public and add `pub use onepassword::OnePasswordBackend;` or direct import path.

  **Must NOT do**:
  - NEVER pass password as a command-line argument to `op`
  - Do not use `op item get` by title (fuzzy match) — always list+filter by ID
  - Do not add retry logic or caching
  - Do not modify the `KeychainBackend` trait
  - Do not add timeout handling

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Core implementation with security-sensitive subprocess management, trait implementation, and careful error handling
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `systematic-debugging`: Not debugging, building new code

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (with Task 5, but Task 5 depends on this)
  - **Blocks**: Tasks 5, 6, 7
  - **Blocked By**: Tasks 1, 2

  **References**:

  **Pattern References**:
  - `src/keychain.rs:18-104` — `RealKeychainBackend` trait implementation — follow this structure (entry helper, error mapping)
  - `src/keychain.rs:51-104` — Store/get/delete/list method patterns — match error message format exactly
  - `src/keychain.rs:65-77` — `get()` error handling pattern — `"key not found: {key}"` format is critical

  **API/Type References**:
  - `src/keychain.rs:11-16` — `KeychainBackend` trait signature — all 4 methods
  - `src/error.rs:38-54` — `SshpassError` enum — use `KeychainAccess(String)` variant
  - `src/password.rs:9-11` — `PasswordSource` trait — `resolve()` returns `SecretString`

  **Test References**:
  - `src/keychain.rs:396-404` — `test_inmemory_store_and_get` — assertion pattern for store+get round-trip

  **External References**:
  - `op item create - --format json [--vault V]` stdin format: `{"title":"key","category":"PASSWORD","tags":["sshpass-rs"],"fields":[{"id":"password","type":"CONCEALED","value":"secret"}]}` — the `-` argument tells `op` to read the item template from stdin
  - `op item list --tags sshpass-rs --format json` — tag-filtered listing
  - `op item get <id> --format json` — get by ID (NOT by title)
  - `op item delete <id>` — delete by ID

  **WHY Each Reference Matters**:
  - `src/keychain.rs:65-77` — The `get()` error message format `"key not found: {key}"` is relied upon by callers. 1Password backend MUST produce identical error strings.
  - `src/keychain.rs:51-104` — Shows the store→index-update pattern. 1Password backend uses tags instead of an index, which is cleaner.
  - `src/error.rs:38-54` — All errors from this module MUST use `SshpassError::KeychainAccess`.

  **Acceptance Criteria**:
  - [ ] `OnePasswordBackend` struct exists with `vault` and `op_path` fields
  - [ ] All 4 `KeychainBackend` trait methods implemented
  - [ ] `store()` pipes password via stdin JSON, not CLI args
  - [ ] `get()` uses list+filter approach, not `op item get` by title
  - [ ] `delete()` resolves title→ID then deletes by ID
  - [ ] `run_op()` catches `io::ErrorKind::NotFound` with helpful message
  - [ ] `run_op()` includes stderr in error messages for non-zero exit codes
  - [ ] `cargo check` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Backend struct compiles and implements trait
    Tool: Bash (cargo check)
    Preconditions: Tasks 1, 2 complete
    Steps:
      1. Run `cargo check`
      2. Assert exit code 0
      3. Assert no errors related to OnePasswordBackend or KeychainBackend trait
    Expected Result: Clean compilation with trait fully implemented
    Failure Indicators: "method not found", "not satisfied" trait errors
    Evidence: .sisyphus/evidence/task-4-trait-compile.txt

  Scenario: store() does not leak password in command args
    Tool: Bash (cargo test)
    Preconditions: Task 4 implementation complete
    Steps:
      1. Run unit test that captures the args passed to `op` subprocess
      2. Assert the word "password_value" does NOT appear in any arg
      3. Assert stdin_data DOES contain the password value in JSON
    Expected Result: Password only in stdin, never in args
    Failure Indicators: Password string found in command arguments
    Evidence: .sisyphus/evidence/task-4-no-password-leak.txt
  ```

  **Commit**: YES
  - Message: `feat(1password): implement OnePasswordBackend with KeychainBackend trait`
  - Files: `src/onepassword.rs`, `src/main.rs`
  - Pre-commit: `cargo check`

- [x] 4b. Fix KeychainPassword::resolve() error handling for backend failures

  **What to do**:
  - In `src/password.rs`, modify `KeychainPassword::resolve()` (lines 172-178) to distinguish between "key not found" (fallback to prompting) and "backend operational failure" (propagate error).
  - Current behavior: ANY `backend.get()` error triggers `prompt_and_maybe_save()` fallback.
  - Required behavior:
    - If error message contains `"key not found"` → fallback to interactive prompt (existing behavior)
    - If error is any OTHER `KeychainAccess` error (e.g., "1Password CLI (op) not found", "op exited with status 1") → propagate the error immediately, do NOT prompt
  - This is critical because without this fix, `SSHPASS_RS_BACKEND=op` with a missing `op` binary will silently fall back to prompting instead of surfacing the "1Password CLI (op) not found" error.
  - Modify the `match self.backend.get(&self.key)` block:
    ```
    match self.backend.get(&self.key) {
        Ok(password) => Ok(password),
        Err(SshpassError::KeychainAccess(msg)) if msg.starts_with("key not found:") => {
            self.prompt_and_maybe_save()
        }
        Err(e) => Err(e),  // Propagate backend operational failures
    }
    ```

  **Must NOT do**:
  - Do not change the KeychainBackend trait
  - Do not change the happy path behavior
  - Do not change the "key not found" fallback behavior
  - Do not modify any other function in password.rs

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Behavior-sensitive change in error handling — must preserve existing key-not-found fallback while enabling new error propagation
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `systematic-debugging`: Not debugging, fixing error propagation path

  **Parallelization**:
  - **Can Run In Parallel**: NO (must be done alongside or after Task 4)
  - **Parallel Group**: Wave 2 (with Task 4, sequential after)
  - **Blocks**: Tasks 5, 7
  - **Blocked By**: Task 4

  **References**:

  **Pattern References**:
  - `src/password.rs:172-178` — Current `KeychainPassword::resolve()` — the match block to modify
  - `src/keychain.rs:65-77` — `RealKeychainBackend::get()` — produces `"key not found: {key}"` on missing entries

  **API/Type References**:
  - `src/error.rs:49` — `SshpassError::KeychainAccess(String)` — the variant to match against

  **Test References**:
  - `src/password.rs:389-398` — `test_keychain_hit` — must still pass (happy path)
  - `src/password.rs:400-414` — `test_keychain_miss_with_test_password` — must still pass (key-not-found fallback)

  **External References**:
  - None

  **WHY Each Reference Matters**:
  - `src/password.rs:172-178` — This is THE code to modify. The `Err(_)` catch-all must become discriminated.
  - `src/keychain.rs:65-77` — Confirms the error message format for "key not found" that we match against.

  **Acceptance Criteria**:
  - [ ] `KeychainPassword::resolve()` propagates non-"key not found" errors
  - [ ] "key not found" errors still trigger prompt fallback
  - [ ] All existing keychain password tests pass
  - [ ] `cargo test -- password` passes

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Backend failure propagates (does not prompt)
    Tool: Bash (cargo test)
    Preconditions: Task 4 complete, OnePasswordBackend available
    Steps:
      1. Create unit test: construct KeychainPassword with a backend that returns `SshpassError::KeychainAccess("1Password CLI (op) not found...")`
      2. Call resolve()
      3. Assert it returns Err, NOT Ok (did not fallback to prompt)
      4. Assert error message contains "1Password CLI (op) not found"
    Expected Result: Backend failure propagated without prompting
    Failure Indicators: Test prompts for password instead of returning error
    Evidence: .sisyphus/evidence/task-4b-error-propagation.txt

  Scenario: Key-not-found still triggers prompt fallback
    Tool: Bash (cargo test)
    Preconditions: Existing tests
    Steps:
      1. Run `cargo test -- password::tests::test_keychain_miss_with_test_password`
      2. Assert test passes (key-not-found still falls back to prompt)
    Expected Result: Existing fallback behavior preserved
    Failure Indicators: Test fails, prompt no longer triggered on key-not-found
    Evidence: .sisyphus/evidence/task-4b-fallback-preserved.txt
  ```

  **Commit**: YES
  - Message: `fix(password): propagate backend operational errors instead of prompting`
  - Files: `src/password.rs`
  - Pre-commit: `cargo test`

- [x] 5. Wire backend selection in build_keychain_backend()

  **What to do**:
  - Modify `build_keychain_backend()` in `src/main.rs` to check `SSHPASS_RS_BACKEND` env var:
    1. If `SSHPASS_RS_BACKEND` is `"op"` or `"1password"`: create `OnePasswordBackend::new(vault)` where `vault` is read from `SSHPASS_RS_VAULT` env var (None if unset)
    2. Else if `SSHPASS_RS_TEST_KEYCHAIN_FILE` is set: existing `FileKeychainBackend` (unchanged)
    3. Else: existing `RealKeychainBackend` (unchanged)
  - Add `use crate::onepassword::OnePasswordBackend;` import to `src/main.rs`
  - Ensure `SSHPASS_RS_VAULT` is ONLY read when 1Password backend is selected

  **Must NOT do**:
  - Do not change any existing behavior when `SSHPASS_RS_BACKEND` is unset
  - Do not modify `handle_standalone()`, `resolve_password()`, or any other function
  - Do not add new CLI flags for backend selection
  - Do not read `SSHPASS_RS_VAULT` unless 1Password backend is active

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Modifying a wiring function with careful env var precedence and backward compatibility
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `systematic-debugging`: Not debugging

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (after Task 4)
  - **Blocks**: Task 7
  - **Blocked By**: Task 4

  **References**:

  **Pattern References**:
  - `src/main.rs:91-96` — Current `build_keychain_backend()` — the function to modify
  - `src/main.rs:80-82` — `build_keychain_manager()` calls `build_keychain_backend()` — must still work

  **API/Type References**:
  - `src/keychain.rs:11-16` — `KeychainBackend` trait — return type is `Box<dyn KeychainBackend>`
  - `src/main.rs:11` — Current imports from keychain module

  **Test References**:
  - `src/main.rs:210-282` — Existing integration tests — these must continue to pass unchanged

  **External References**:
  - None

  **WHY Each Reference Matters**:
  - `src/main.rs:91-96` — This is THE function being modified. Must understand its current logic (env var check → file backend / real backend)
  - `src/main.rs:80-82` — `build_keychain_manager()` calls this function, so the return type and behavior contract must be preserved

  **Acceptance Criteria**:
  - [ ] `SSHPASS_RS_BACKEND=op` returns `OnePasswordBackend`
  - [ ] `SSHPASS_RS_BACKEND=1password` returns `OnePasswordBackend`
  - [ ] `SSHPASS_RS_BACKEND` unset → existing behavior unchanged
  - [ ] `SSHPASS_RS_VAULT=MyVault` with `SSHPASS_RS_BACKEND=op` → vault set
  - [ ] `SSHPASS_RS_VAULT` without `SSHPASS_RS_BACKEND=op` → ignored
  - [ ] `cargo test` — all existing tests pass

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Backend selection with SSHPASS_RS_BACKEND=op (behavior-based)
    Tool: Bash (cargo test)
    Preconditions: Tasks 1-4 complete
    Steps:
      1. Run unit test that sets `SSHPASS_RS_BACKEND=op` and calls `build_keychain_backend()`
      2. Call `list()` on the returned backend — it should attempt to run `op` (not access macOS Keychain)
      3. Since `op` is not installed in test env, assert the error contains "1Password CLI (op) not found" — this proves it's the OnePasswordBackend, not RealKeychainBackend
      4. Run same test with `SSHPASS_RS_BACKEND=1password` — same behavior
    Expected Result: Backend attempts to call `op` CLI, confirming OnePasswordBackend was selected
    Failure Indicators: Error mentions "keyring" or macOS Keychain instead of "1Password CLI"
    Evidence: .sisyphus/evidence/task-5-backend-selection.txt

  Scenario: Existing behavior preserved when env var unset
    Tool: Bash (cargo test)
    Preconditions: Tasks 1-4 complete
    Steps:
      1. Unset `SSHPASS_RS_BACKEND`
      2. Run full existing test suite: `cargo test`
      3. Assert all tests pass — zero regressions
    Expected Result: All existing tests pass, no behavior change
    Failure Indicators: Any test failure, especially in integration tests
    Evidence: .sisyphus/evidence/task-5-regression.txt
  ```

  **Commit**: YES
  - Message: `feat(1password): wire backend selection via SSHPASS_RS_BACKEND env var`
  - Files: `src/main.rs`
  - Pre-commit: `cargo test`

- [x] 6. Add unit tests for OnePasswordBackend operations

  **What to do**:
  - Add unit tests in `src/onepassword.rs` `#[cfg(test)] mod tests` block:
    - **`test_store_constructs_correct_command`**: Verify `store()` builds correct `op item create` args with `- --format json [--vault V]` (trailing `-` for stdin template) and stdin JSON contains the password value (never in args)
    - **`test_store_without_vault_omits_flag`**: Verify `--vault` is omitted when `vault` is `None`
    - **`test_get_returns_password`**: With mock, verify `get("user@host")` calls list first, then get by ID, returns correct `SecretString`
    - **`test_get_not_found`**: Verify `get("nonexistent")` returns `SshpassError::KeychainAccess("key not found: nonexistent")`
    - **`test_get_exact_match`**: Verify `get("prod")` does NOT match `"prod-server"` — exact title match only
    - **`test_delete_resolves_id`**: Verify `delete("user@host")` calls list to get ID, then deletes by ID
    - **`test_delete_not_found`**: Verify `delete("nonexistent")` returns `SshpassError::KeychainAccess("key not found: nonexistent")`
    - **`test_list_returns_titles`**: Verify `list()` returns `Vec<String>` of item titles
    - **`test_list_empty`**: Verify `list()` returns empty vec for `[]`
    - **`test_op_not_found`**: Verify error message contains "1Password CLI (op) not found" when op binary doesn't exist
    - **`test_op_stderr_included`**: Verify non-zero exit code includes stderr in error message
  - Use `OnePasswordBackend::with_op_path()` to inject a mock/non-existent binary path for tests

  **Must NOT do**:
  - Do not test against a real `op` CLI installation
  - Do not modify existing tests in `src/keychain.rs`

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Comprehensive test suite with mock injection and error case coverage
  - **Skills**: [`test-driven-development`]
    - `test-driven-development`: Useful for structuring test scenarios and assertions

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Task 7, Task 8)
  - **Parallel Group**: Wave 3
  - **Blocks**: Task 7 (indirectly — tests should pass before integration tests)
  - **Blocked By**: Task 4

  **References**:

  **Pattern References**:
  - `src/keychain.rs:392-468` — `InMemoryKeychainBackend` unit tests — assertion style, error matching patterns
  - `src/keychain.rs:450-466` — `test_inmemory_get_nonexistent` — error message assertion pattern
  - `src/password.rs:216-493` — Password source unit tests — `ENV_MUTEX` pattern for env var tests

  **API/Type References**:
  - `src/error.rs:38-54` — `SshpassError` enum — match against `KeychainAccess` variant
  - `secrecy::ExposeSecret` — to assert `SecretString` content

  **Test References**:
  - `src/password.rs:224` — `static ENV_MUTEX: Mutex<()>` — thread-safe env var testing pattern, copy this

  **External References**:
  - None

  **WHY Each Reference Matters**:
  - `src/keychain.rs:450-466` — The exact error assertion pattern (`match result.unwrap_err()`) must be followed
  - `src/password.rs:224` — ENV_MUTEX is critical for tests that modify env vars to avoid race conditions

  **Acceptance Criteria**:
  - [ ] All 11 unit tests exist and pass
  - [ ] `cargo test -- onepassword` runs all tests successfully
  - [ ] Tests use `with_op_path()` for mock injection, not real `op` binary
  - [ ] Error message format tested: `"key not found: {key}"` matches exactly

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: All unit tests pass
    Tool: Bash (cargo test)
    Preconditions: Tasks 1-4 complete
    Steps:
      1. Run `cargo test -- onepassword 2>&1`
      2. Assert exit code 0
      3. Assert output contains "test result: ok" with 0 failures
      4. Count test names — assert >= 11 tests ran
    Expected Result: All onepassword tests pass
    Failure Indicators: Any test failure, fewer than 11 tests
    Evidence: .sisyphus/evidence/task-6-unit-tests.txt

  Scenario: Existing tests unaffected
    Tool: Bash (cargo test)
    Preconditions: Task 6 complete
    Steps:
      1. Run full `cargo test 2>&1`
      2. Assert exit code 0
      3. Assert no failures in any test module
    Expected Result: Zero regressions
    Failure Indicators: Any failure in keychain, password, cli, or integration tests
    Evidence: .sisyphus/evidence/task-6-regression.txt
  ```

  **Commit**: YES
  - Message: `test(1password): add unit tests for backend operations`
  - Files: `src/onepassword.rs`
  - Pre-commit: `cargo test`

- [x] 7. Add integration tests with mock op binary

  **What to do**:
  - Create `tests/onepassword_integration.rs` with integration tests using the mock `op` script from Task 3
  - Test setup:
    - Set `SSHPASS_RS_BACKEND=op` env var
    - Set `PATH` to include `tests/fixtures/` directory (so `mock_op.sh` is found as `op`) OR use a wrapper that renames mock_op.sh to `op` in a temp directory
    - Alternatively, if `OnePasswordBackend` supports `op_path` injection at the integration level, use that
  - Integration test scenarios:
    - **Full CRUD round-trip**: `--store user@host` → `--list` (shows `user@host`) → `--delete user@host` → `--list` (empty). NOTE: The `-k` password resolution flow requires a mock SSH target (`target/debug/fake_ssh --mode success`) — use the same pattern as `tests/integration.rs:253-266`.
    - **Password resolution via `-k`**: Set `SSHPASS_RS_BACKEND=op` + inject mock `op` path, run `sshpass-rs -k target/debug/fake_ssh --mode success` (uses `fake_ssh` binary, NOT real `ssh`), assert exit code 0 and stdout contains "Welcome!"
    - **Error: `op` not installed**: Set `SSHPASS_RS_BACKEND=op` + set op_path to nonexistent binary via env, run `sshpass-rs -k target/debug/fake_ssh --mode success`, assert exit code 3 and stderr contains "1Password CLI (op) not found"
    - **Error: `op` returns error**: Mock returns exit 1 + stderr, run `sshpass-rs --list` with `SSHPASS_RS_BACKEND=op`, assert sshpass-rs exit code 3 and stderr contains the `op` error
  - Use `assert_cmd::Command` and `predicates` crate following existing integration test patterns

  **Must NOT do**:
  - Do not require real 1Password CLI or real vault for tests
  - Do not modify existing `tests/integration.rs`
  - Do not test against real SSH connections

  **Recommended Agent Profile**:
  - **Category**: `unspecified-high`
    - Reason: Integration test setup with mock binary injection and env var manipulation
  - **Skills**: [`test-driven-development`]
    - `test-driven-development`: Test design and structure guidance

  **Parallelization**:
  - **Can Run In Parallel**: NO (depends on Tasks 3, 5, 6)
  - **Parallel Group**: Wave 3 (after Tasks 5, 6)
  - **Blocks**: None
  - **Blocked By**: Tasks 3, 5, 6

  **References**:

  **Pattern References**:
  - `tests/integration.rs:216-233` — `temp_keychain_env()` helper and env var injection pattern for test isolation
  - `tests/integration.rs:223-233` — `list_standalone_prints_empty_for_fresh_store` — assert_cmd usage with env var and predicate
  - `tests/integration.rs:236-249` — Error exit code assertion pattern

  **API/Type References**:
  - `assert_cmd::Command` — `Command::cargo_bin("sshpass-rs")` for running the binary
  - `predicates::prelude::*` — `predicate::str::contains(...)` for output assertions

  **Test References**:
  - `tests/integration.rs:253-266` — `successful_fake_ssh_flow_exits_zero` — successful flow pattern using `fake_ssh`
  - `src/bin/fake_ssh.rs` — Existing mock binary — shows how the project approaches test mocks

  **External References**:
  - None

  **WHY Each Reference Matters**:
  - `tests/integration.rs:216-233` — The exact pattern for test isolation (tempdir + env var injection) must be followed for 1Password integration tests
  - `tests/integration.rs:253-266` — Shows how to test a full sshpass-rs flow end-to-end with a mock target binary

  **Acceptance Criteria**:
  - [ ] `tests/onepassword_integration.rs` exists with >= 3 integration tests
  - [ ] `cargo test --test onepassword_integration` passes all tests
  - [ ] Tests use mock `op` script, not real 1Password CLI
  - [ ] Error paths tested (op not found, op returns error)

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Integration tests pass with mock op
    Tool: Bash (cargo test)
    Preconditions: Tasks 1-6 complete, mock_op.sh exists, `cargo build` done (for fake_ssh binary)
    Steps:
      1. Run `cargo build 2>&1` — ensure fake_ssh and sshpass-rs binaries are built
      2. Run `cargo test --test onepassword_integration 2>&1`
      3. Assert exit code 0
      4. Assert output contains "test result: ok" with 0 failures
    Expected Result: All integration tests pass using mock op + fake_ssh
    Failure Indicators: Any test failure, mock_op.sh not found, fake_ssh not built
    Evidence: .sisyphus/evidence/task-7-integration.txt

  Scenario: Password resolution e2e with mock op and fake_ssh
    Tool: Bash
    Preconditions: mock_op.sh configured to return password for "user@host", fake_ssh built
    Steps:
      1. Set `SSHPASS_RS_BACKEND=op` + inject mock op path
      2. Run `./target/debug/sshpass-rs -k target/debug/fake_ssh --mode success`
      3. Assert exit code 0
      4. Assert stdout contains "Welcome!" (fake_ssh success output)
    Expected Result: Full flow: backend selection → op invocation → password retrieval → PTY handshake → success
    Failure Indicators: Exit code != 0, missing "Welcome!", keychain error instead of op error
    Evidence: .sisyphus/evidence/task-7-e2e-flow.txt

  Scenario: Full test suite passes (all tests together)
    Tool: Bash (cargo test)
    Preconditions: Tasks 1-7 complete
    Steps:
      1. Run `cargo test 2>&1`
      2. Assert exit code 0
      3. Run `cargo clippy -- -D warnings 2>&1`
      4. Assert exit code 0
      5. Run `cargo fmt --check 2>&1`
      6. Assert exit code 0
    Expected Result: Full clean build + test + lint + format pass
    Failure Indicators: Any failure in any check
    Evidence: .sisyphus/evidence/task-7-full-suite.txt
  ```

  **Commit**: YES
  - Message: `test(1password): add integration tests with mock op binary`
  - Files: `tests/onepassword_integration.rs`, `tests/fixtures/mock_op.sh`
  - Pre-commit: `cargo test`

- [x] 8. Update README with 1Password documentation

  **What to do**:
  - Add a new section to `README.md` after the "Keychain management flags" section:
    - **1Password backend** section explaining:
      - Environment variables: `SSHPASS_RS_BACKEND`, `SSHPASS_RS_VAULT`
      - How to enable: `export SSHPASS_RS_BACKEND=op`
      - Prerequisites: 1Password CLI (`op`) must be installed and authenticated
      - Usage examples:
        - Store: `SSHPASS_RS_BACKEND=op sshpass-rs --store user@host`
        - Use: `SSHPASS_RS_BACKEND=op sshpass-rs -k ssh user@host`
        - List: `SSHPASS_RS_BACKEND=op sshpass-rs --list`
        - Delete: `SSHPASS_RS_BACKEND=op sshpass-rs --delete user@host`
      - With specific vault: `SSHPASS_RS_VAULT=Production SSHPASS_RS_BACKEND=op sshpass-rs -k ssh user@host`
      - Service account for automation: `export OP_SERVICE_ACCOUNT_TOKEN=ops_...`
  - Update the "New in sshpass-rs" section to mention 1Password support
  - Update the Security table to add 1Password row
  - Update the Limitations section to note `op` CLI dependency

  **Must NOT do**:
  - Do not modify existing examples or flag documentation
  - Do not add excessive detail about 1Password CLI installation (link to official docs instead)
  - Do not document internal implementation details

  **Recommended Agent Profile**:
  - **Category**: `writing`
    - Reason: Documentation writing, no code changes
  - **Skills**: []
  - **Skills Evaluated but Omitted**:
    - `frontend-design`: Not a UI task

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 6, 7)
  - **Parallel Group**: Wave 3
  - **Blocks**: None
  - **Blocked By**: None (can reference decisions, no code dependency)

  **References**:

  **Pattern References**:
  - `README.md:36-66` — Existing flag documentation format (table + examples)
  - `README.md:139-149` — "Recommended workflow" section — follow this pattern for 1Password workflow
  - `README.md:196-214` — Security table format

  **API/Type References**:
  - None

  **Test References**:
  - None

  **External References**:
  - 1Password CLI installation: https://1password.com/downloads/command-line/
  - 1Password service accounts: https://developer.1password.com/docs/service-accounts/

  **WHY Each Reference Matters**:
  - `README.md:36-66` — Documentation style and table format must be consistent
  - `README.md:139-149` — Workflow pattern to follow for 1Password equivalent

  **Acceptance Criteria**:
  - [ ] "1Password backend" section exists in README
  - [ ] Environment variables documented: `SSHPASS_RS_BACKEND`, `SSHPASS_RS_VAULT`
  - [ ] Usage examples cover store, use, list, delete with 1Password
  - [ ] Security table updated with 1Password row
  - [ ] Limitations section mentions `op` CLI dependency

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: README contains 1Password documentation
    Tool: Bash (grep)
    Preconditions: Task 8 complete
    Steps:
      1. Run `grep -c "1Password" README.md`
      2. Assert count >= 5 (appears in multiple sections)
      3. Run `grep -c "SSHPASS_RS_BACKEND" README.md`
      4. Assert count >= 3
      5. Run `grep -c "SSHPASS_RS_VAULT" README.md`
      6. Assert count >= 2
    Expected Result: 1Password documented thoroughly in README
    Failure Indicators: Missing sections, undocumented env vars
    Evidence: .sisyphus/evidence/task-8-readme.txt
  ```

  **Commit**: YES
  - Message: `docs: document 1Password backend usage and environment variables`
  - Files: `README.md`
  - Pre-commit: `cargo test`

---

## Final Verification Wave (MANDATORY — after ALL implementation tasks)

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.
>
> **Do NOT auto-proceed after verification. Wait for user's explicit approval before marking work complete.**

- [x] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (read file, run command). For each "Must NOT Have": search codebase for forbidden patterns — reject with file:line if found. Check evidence files exist in .sisyphus/evidence/. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [N/N] | VERDICT: APPROVE/REJECT`

  **QA Scenarios:**
  ```
  Scenario: Must Have verification
    Tool: Bash + Read
    Steps:
      1. Read `src/onepassword.rs` — assert `pub struct OnePasswordBackend` exists
      2. Run `grep -n "KeychainBackend" src/onepassword.rs` — assert `impl KeychainBackend for OnePasswordBackend` found
      3. Run `grep -n "SSHPASS_RS_BACKEND" src/main.rs` — assert env var check exists in `build_keychain_backend()`
      4. Run `grep -rn "op_path" src/onepassword.rs` — assert injectable op_path field exists
      5. Run `grep -rn "stdin" src/onepassword.rs` — assert stdin piping exists (not CLI arg passwords)
      6. Run `grep -c "1Password" README.md` — assert >= 5
      7. Run `ls .sisyphus/evidence/task-*.txt` — assert evidence files exist for each task
    Expected Result: All 8 Must Have items verified
    Evidence: .sisyphus/evidence/F1-compliance.txt

  Scenario: Must NOT Have verification
    Tool: Bash (grep)
    Preconditions: Record the starting commit hash BEFORE implementation begins (store in `.sisyphus/evidence/base-commit.txt`)
    Steps:
      1. Read base commit from `.sisyphus/evidence/base-commit.txt` (recorded before work started)
      2. Run `grep -rn "\.args.*password" src/onepassword.rs` — assert password NOT in command args (stdin only)
      3. Run `git diff <base-commit>..HEAD -- src/keychain.rs | grep "^[+-].*fn store\|^[+-].*fn get\|^[+-].*fn delete\|^[+-].*fn list"` — assert `KeychainBackend` trait signature unchanged (only impl additions, no trait changes)
      4. Run `git diff <base-commit>..HEAD -- src/pty.rs src/matcher.rs src/signals.rs` — assert empty diff (no changes to these files)
      5. Run `grep -rn "feature.*onepassword\|cfg.*onepassword" Cargo.toml src/` — assert no feature flags
    Expected Result: All 13 Must NOT Have items verified absent
    Evidence: .sisyphus/evidence/F1-must-not-have.txt
  ```

- [x] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo check && cargo clippy -- -D warnings && cargo fmt --check && cargo test`. Review all changed files for: `as any`/unwrap-without-reason, empty matches, debug prints in prod, commented-out code, unused imports. Check AI slop: excessive comments, over-abstraction, generic names.
  Output: `Build [PASS/FAIL] | Clippy [PASS/FAIL] | Tests [N pass/N fail] | Files [N clean/N issues] | VERDICT`

  **QA Scenarios:**
  ```
  Scenario: Full build + lint + test pipeline
    Tool: Bash
    Steps:
      1. Run `cargo check 2>&1` — assert exit code 0
      2. Run `cargo clippy -- -D warnings 2>&1` — assert exit code 0
      3. Run `cargo fmt --check 2>&1` — assert exit code 0
      4. Run `cargo test 2>&1` — assert exit code 0, capture test count
      5. Run `grep -rn "unwrap()" src/onepassword.rs | grep -v "test" | grep -v "#\[cfg(test)\]"` — assert 0 non-test unwraps
      6. Run `grep -rn "println!" src/onepassword.rs | grep -v "test" | grep -v "#\[cfg(test)\]"` — assert 0 non-test printlns
      7. Run `grep -rn "// TODO\|// FIXME\|// HACK" src/onepassword.rs` — assert 0 unresolved TODOs
    Expected Result: Clean build, zero warnings, all tests pass, no code smells
    Evidence: .sisyphus/evidence/F2-quality.txt
  ```

- [x] F3. **Real Manual QA** — `unspecified-high`
  Start from clean state. Execute EVERY QA scenario from EVERY task — follow exact steps, capture evidence. Test cross-task integration (backend selection + CRUD). Test edge cases: `op` not installed, invalid vault, empty list. Save to `.sisyphus/evidence/final-qa/`.
  Output: `Scenarios [N/N pass] | Integration [N/N] | Edge Cases [N tested] | VERDICT`

  **QA Scenarios:**
  ```
  Scenario: Cross-task integration — backend selection + list
    Tool: Bash
    Steps:
      1. Build: `cargo build 2>&1` — assert exit code 0
      2. Set `SSHPASS_RS_BACKEND=op` — do NOT install real `op`
      3. Run `./target/debug/sshpass-rs --list 2>&1`
      4. Assert exit code 3 (RuntimeError) and stderr contains "1Password CLI (op) not found"
    Expected Result: Backend selected, op invoked, clear error when missing
    Evidence: .sisyphus/evidence/final-qa/cross-task-backend-list.txt

  Scenario: Edge case — empty SSHPASS_RS_BACKEND value
    Tool: Bash
    Steps:
      1. Set `SSHPASS_RS_BACKEND=` (empty string)
      2. Set `SSHPASS_RS_TEST_KEYCHAIN_FILE=/tmp/test_kc.json`
      3. Run `./target/debug/sshpass-rs --list 2>&1`
      4. Assert falls through to FileKeychainBackend (prints "(empty)" for fresh file)
    Expected Result: Empty env var treated as unset, defaults to existing behavior
    Evidence: .sisyphus/evidence/final-qa/edge-empty-backend.txt

  Scenario: Edge case — invalid SSHPASS_RS_BACKEND value
    Tool: Bash
    Steps:
      1. Set `SSHPASS_RS_BACKEND=invalid`
      2. Run `./target/debug/sshpass-rs --list 2>&1`
      3. Assert behavior: either falls through to default or returns clear error
    Expected Result: Graceful handling of unknown backend value
    Evidence: .sisyphus/evidence/final-qa/edge-invalid-backend.txt

  Scenario: Regression — existing keychain tests unaffected
    Tool: Bash
    Steps:
      1. Unset `SSHPASS_RS_BACKEND`
      2. Run `cargo test --test integration 2>&1`
      3. Assert all existing integration tests pass, zero failures
    Expected Result: Zero regressions in existing test suite
    Evidence: .sisyphus/evidence/final-qa/regression-integration.txt
  ```

- [x] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual diff (git log/diff). Verify 1:1 — everything in spec was built (no missing), nothing beyond spec was built (no creep). Check "Must NOT do" compliance. Detect cross-task contamination: Task N touching Task M's files. Flag unaccounted changes.
  Output: `Tasks [N/N compliant] | Contamination [CLEAN/N issues] | Unaccounted [CLEAN/N files] | VERDICT`

  **QA Scenarios:**
  ```
  Scenario: Scope fidelity — files touched per task
    Tool: Bash (git log)
    Preconditions: Base commit hash recorded in `.sisyphus/evidence/base-commit.txt` before work started
    Steps:
      1. Read base commit from `.sisyphus/evidence/base-commit.txt`
      2. Run `git log <base-commit>..HEAD --oneline --name-only` for commits in this work only
      3. For commit "feat(deps):" — assert ONLY `Cargo.toml` and `Cargo.lock` changed
      4. For commit "feat(1password): add op CLI JSON" — assert ONLY `src/onepassword.rs`, `src/main.rs` changed
      5. For commit "feat(1password): implement" — assert ONLY `src/onepassword.rs`, `src/main.rs` changed
      6. For commit "fix(password): propagate" — assert ONLY `src/password.rs` changed
      7. For commit "feat(1password): wire" — assert ONLY `src/main.rs` changed
      8. For commit "test(1password): unit" — assert ONLY `src/onepassword.rs` changed
      9. For commit "test(1password): integration" — assert ONLY `tests/onepassword_integration.rs`, `tests/fixtures/mock_op.sh` changed
      10. For commit "docs:" — assert ONLY `README.md` changed
      11. Run `git diff <base-commit>..HEAD -- src/pty.rs src/matcher.rs src/signals.rs src/cli.rs` — assert empty diff
    Expected Result: Each commit touches only its declared files, no cross-contamination
    Evidence: .sisyphus/evidence/F4-scope.txt

  Scenario: Must NOT do compliance
    Tool: Bash (grep)
    Steps:
      1. Run `grep -rn "op item get.*--title\|op item get.*\"[^\"]*\".*--format" src/onepassword.rs` — assert no `op item get` by title (fuzzy match forbidden)
      2. Run `grep -rn "timeout\|Timeout\|TIMEOUT" src/onepassword.rs` — assert no timeout handling
      3. Run `grep -rn "retry\|Retry\|RETRY" src/onepassword.rs` — assert no retry logic
      4. Run `grep -rn "cache\|Cache\|CACHE" src/onepassword.rs` — assert no caching
    Expected Result: All "Must NOT Have" constraints respected
    Evidence: .sisyphus/evidence/F4-must-not.txt
  ```

---

## Commit Strategy

| Commit | Message | Files | Pre-commit |
|--------|---------|-------|------------|
| 1 | `feat(deps): add serde and serde_json for 1Password JSON parsing` | `Cargo.toml` | `cargo check` |
| 2 | `feat(1password): add op CLI JSON response types and parsing` | `src/onepassword.rs` (new) | `cargo test` |
| 3 | `feat(1password): implement OnePasswordBackend with KeychainBackend trait` | `src/onepassword.rs`, `src/main.rs` (mod declaration) | `cargo test` |
| 4 | `fix(password): propagate backend operational errors instead of prompting` | `src/password.rs` | `cargo test` |
| 5 | `feat(1password): wire backend selection via SSHPASS_RS_BACKEND env var` | `src/main.rs` | `cargo test` |
| 6 | `test(1password): add unit tests for backend operations` | `src/onepassword.rs` | `cargo test` |
| 7 | `test(1password): add integration tests with mock op binary` | `tests/onepassword_integration.rs`, test fixtures | `cargo test` |
| 8 | `docs: document 1Password backend usage and environment variables` | `README.md` | `cargo test` |

---

## Success Criteria

### Verification Commands
```bash
cargo check              # Expected: compiles cleanly
cargo test               # Expected: all tests pass (existing + new)
cargo clippy -- -D warnings  # Expected: no warnings
cargo fmt --check        # Expected: no formatting issues
```

### Final Checklist
- [ ] All "Must Have" present
- [ ] All "Must NOT Have" absent
- [ ] All tests pass
- [ ] Existing tests unmodified and passing
- [ ] `SSHPASS_RS_BACKEND` unset → identical behavior to before

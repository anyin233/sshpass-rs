# sshpass-rs

A drop-in Rust replacement for [sshpass](https://sourceforge.net/projects/sshpass/) with secure password storage via macOS Keychain and 1Password.

## Quick Start

```sh
# Install
cargo install --path .

# Store a password once, use it everywhere
sshpass-rs --store user@host        # prompts for password, saves to Keychain
sshpass-rs -k ssh user@host         # looks up stored password automatically
sshpass-rs -k scp user@host:f .     # works with scp, rsync, sftp, etc.

# Or pass a password directly (original sshpass style)
sshpass-rs -p mypass ssh user@host
```

**Using 1Password instead of macOS Keychain?**

```sh
export SSHPASS_RS_BACKEND=op
sshpass-rs --store user@host
sshpass-rs -k ssh user@host
```

---

## Installation

```sh
# From source
cargo install --path .

# Or build manually
cargo build --release
# Binary at: target/release/sshpass-rs
```

## Configuration

### Backend Selection

sshpass-rs supports two password storage backends, controlled by environment variables:

| Variable | Values | Description |
|----------|--------|-------------|
| `SSHPASS_RS_BACKEND` | `op`, `1password` | Use 1Password instead of macOS Keychain. Unset = macOS Keychain. |
| `SSHPASS_RS_VAULT` | vault name | 1Password vault to use (optional, defaults to personal vault) |

**macOS Keychain** (default) — passwords are stored in the system Keychain, encrypted at rest, protected by your login password.

**1Password** — passwords are stored as tagged items in your 1Password vault. Requires the [1Password CLI (`op`)](https://1password.com/downloads/command-line/) to be installed and authenticated.

> On non-macOS platforms, macOS Keychain is unavailable. Set `SSHPASS_RS_BACKEND=op` to use 1Password instead.

#### 1Password Service Account (for automation)

```sh
export OP_SERVICE_ACCOUNT_TOKEN=ops_...
export SSHPASS_RS_BACKEND=op
sshpass-rs -k ssh user@host
```

### Verbose / Diagnostic Output

Use `-v` to see which backend is selected, what commands are executed, and how password resolution proceeds:

```sh
sshpass-rs -v -k ssh user@host
```

```
SSHPASS_RS: checking SSHPASS_RS_BACKEND environment variable
SSHPASS_RS: selected OS keychain backend
SSHPASS_RS: using keychain with key 'user@host'
SSHPASS_RS: querying backend for key 'user@host'
SSHPASS_RS: key 'user@host' found in backend
SSHPASS searching for password prompt using match "assword:"
SSHPASS detected password prompt
SSHPASS sending password
```

## Usage

```
sshpass-rs [OPTIONS] <command> [args...]
sshpass-rs --store <key>
sshpass-rs --delete <key>
sshpass-rs --list
sshpass-rs --help
```

### Password Source Flags

Mutually exclusive — only one per invocation.

| Flag | Description |
|------|-------------|
| `-p <password>` | Pass the password directly as an argument |
| `-f <filename>` | Read the password from a file (first line) |
| `-d <number>` | Read the password from a file descriptor |
| `-e` | Read the password from the `SSHPASS` environment variable |
| `-k` | Look up the password from the configured backend, auto-deriving the key from the SSH command |

No flag = read password from stdin (original sshpass behavior).

### Password Management

Standalone operations — no wrapped command needed.

| Flag | Description |
|------|-------------|
| `--store <key>` | Prompt for a password and store it under `<key>` |
| `--delete <key>` | Delete the stored entry for `<key>` |
| `--list` | List all entries managed by sshpass-rs |
| `--key <value>` | Explicit key name for `-k` (overrides auto-detection) |

### Other Flags

| Flag | Default | Description |
|------|---------|-------------|
| `-P <prompt>` | `assword:` | Prompt pattern to match in PTY output |
| `-v` | off | Verbose mode — diagnostic output to stderr |
| `-h`, `--help` | | Context-sensitive help (try `--store --help`, `--list --help`, `-k --help`) |

### Key Auto-Detection

With `-k` (no `--key`), the key is derived from the wrapped command:

- `ssh user@host` → key = `user@host`
- `ssh -l user host` → key = `user@host`

If neither pattern matches, you're prompted interactively.

## Examples

### Basic Password Passing

```sh
# Direct password
sshpass-rs -p hunter2 ssh user@host

# From file
sshpass-rs -f ~/.ssh/mypassword ssh user@host

# From environment variable
SSHPASS=hunter2 sshpass-rs -e ssh user@host

# From stdin
echo "hunter2" | sshpass-rs ssh user@host
```

### Keychain Workflow

```sh
# Store once
sshpass-rs --store user@host

# Use everywhere
sshpass-rs -k ssh user@host
sshpass-rs -k scp user@host:/remote/file ./local/
sshpass-rs -k rsync -avz user@host:/data/ ./backup/

# Explicit key name
sshpass-rs --key myserver ssh -l user host

# Manage stored entries
sshpass-rs --list
sshpass-rs --delete user@host
```

### 1Password Workflow

```sh
export SSHPASS_RS_BACKEND=op

# Store in 1Password
sshpass-rs --store user@host

# Use from 1Password
sshpass-rs -k ssh user@host

# Use a specific vault
SSHPASS_RS_VAULT=Production sshpass-rs -k ssh user@host

# List / delete
sshpass-rs --list
sshpass-rs --delete user@host
```

### Advanced

```sh
# Custom prompt pattern (non-English systems)
sshpass-rs -p hunter2 -P "Passwort:" ssh user@host

# Verbose diagnostics
sshpass-rs -v -k ssh user@host
```

## Exit Codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | Success | Command completed successfully |
| 1 | InvalidArguments | Missing required argument |
| 2 | ConflictingArguments | Multiple password sources or bad flags |
| 3 | RuntimeError | PTY, spawn, keychain, or I/O error |
| 4 | ParseError | Could not parse child process output |
| 5 | IncorrectPassword | SSH rejected the password |
| 6 | HostKeyUnknown | Host key not in known_hosts |
| 7 | HostKeyChanged | Host key changed since last connection |

## Security

| Property | Original sshpass | sshpass-rs |
|----------|-----------------|------------|
| Password in args (`-p`) | Visible in `ps` | Visible in `ps` (same) |
| Password in file (`-f`) | Plaintext file | Plaintext file (same) |
| Password in env (`-e`) | Cleared before exec | Cleared before exec |
| Keychain storage | Not supported | macOS Keychain (encrypted at rest) |
| 1Password storage | Not supported | `op` CLI (encrypted vault, biometric unlock) |
| In-memory handling | Plain string | `SecretString` (zeroized on drop) |

**Recommendation:** avoid `-p` — the password is visible in `ps`. Use `-k` with stored passwords for scripts and automation.

## Compatibility

### Same as original sshpass

- `-p`, `-f`, `-d`, `-e` password sources
- `-P` prompt pattern matching
- `-v` verbose mode
- Exit codes 0–7
- Works with any PTY-based password prompt (`ssh`, `scp`, `sftp`, `rsync`, etc.)

### New in sshpass-rs

- `-k` / `--key` — backend-backed password lookup
- `--store` / `--delete` / `--list` — password management
- 1Password backend via `SSHPASS_RS_BACKEND=op`
- Auto-detection of `user@host` from SSH arguments
- Interactive fallback when key is missing
- Context-sensitive `--help`
- Cross-platform backend guard (non-macOS → requires 1Password)

## Limitations

- macOS Keychain backend is macOS-only. On other platforms, use `SSHPASS_RS_BACKEND=op`.
- 1Password backend requires the [`op` CLI](https://1password.com/downloads/command-line/).
- No config file — all options via command line and environment variables.

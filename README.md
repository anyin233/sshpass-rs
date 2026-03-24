# sshpass-rs

A Rust replacement for [sshpass](https://sourceforge.net/projects/sshpass/) with macOS Keychain integration.

## Description

`sshpass-rs` wraps SSH commands and supplies passwords non-interactively, just like the original `sshpass`. It's a drop-in replacement for scripts that already use `sshpass`, and it adds one thing the original never had: secure password storage in the macOS Keychain.

Instead of keeping passwords in shell scripts, environment variables, or plaintext files, you store them once in the Keychain and reference them by key. Passwords are held in memory as `SecretString` (zeroized on drop) and the `SSHPASS` environment variable is cleared before the child process starts.

## Installation

Build from source:

```sh
cargo build --release
# Binary at: target/release/sshpass-rs
```

Or install directly into your Cargo bin path:

```sh
cargo install --path .
```

## Usage

### Synopsis

```
sshpass-rs [OPTIONS] <command> [args...]
sshpass-rs --store <key>
sshpass-rs --delete <key>
sshpass-rs --list
```

### Password source flags

These flags are mutually exclusive. Only one may be used per invocation.

| Flag | Description |
|------|-------------|
| `-p <password>` | Pass the password directly as an argument |
| `-f <filename>` | Read the password from a file (first line) |
| `-d <number>` | Read the password from a file descriptor |
| `-e` | Read the password from the `SSHPASS` environment variable |
| `-k` | Look up the password in the macOS Keychain, auto-deriving the key from the wrapped command |

### Other flags

| Flag | Default | Description |
|------|---------|-------------|
| `-P <prompt>` | `assword:` | Prompt pattern to watch for in the PTY output |
| `-v` | off | Verbose mode; prints diagnostic output to stderr |

### Keychain management flags

These are standalone operations. No wrapped command is needed.

| Flag | Description |
|------|-------------|
| `--key <value>` | Explicit Keychain key to use with `-k` (overrides auto-detection) |
| `--store <key>` | Prompt for a password and store it in the Keychain under `<key>` |
| `--delete <key>` | Delete the Keychain entry for `<key>` |
| `--list` | List all Keychain entries managed by sshpass-rs |

### 1Password backend

Set `SSHPASS_RS_BACKEND=op` (or `1password`) to store and retrieve passwords through 1Password instead of the macOS Keychain.

**Prerequisites:** the [1Password CLI (`op`)](https://1password.com/downloads/command-line/) must be installed and signed in.

#### Environment variables

| Variable | Description |
|----------|-------------|
| `SSHPASS_RS_BACKEND` | Set to `op` or `1password` to use 1Password instead of macOS Keychain |
| `SSHPASS_RS_VAULT` | Optional. Specify which 1Password vault to use (defaults to the personal vault) |

#### Usage

```sh
# Store a password in 1Password
SSHPASS_RS_BACKEND=op sshpass-rs --store user@host

# Use a stored 1Password password
SSHPASS_RS_BACKEND=op sshpass-rs -k ssh user@host

# List all sshpass-rs managed passwords in 1Password
SSHPASS_RS_BACKEND=op sshpass-rs --list

# Delete a stored password from 1Password
SSHPASS_RS_BACKEND=op sshpass-rs --delete user@host
```

**With a specific vault:**

```sh
SSHPASS_RS_VAULT=Production SSHPASS_RS_BACKEND=op sshpass-rs -k ssh user@host
```

**Service account for automation (no interactive prompt):**

```sh
export OP_SERVICE_ACCOUNT_TOKEN=ops_...
export SSHPASS_RS_BACKEND=op
sshpass-rs -k ssh user@host
```

Passwords are stored as Password items tagged `sshpass-rs`. Authentication is handled entirely by the `op` CLI (biometric unlock, service account token, or session token).

### Examples

**Pass password directly (original sshpass style):**

```sh
sshpass-rs -p hunter2 ssh user@host
```

**Read password from a file:**

```sh
sshpass-rs -f ~/.ssh/mypassword ssh user@host
```

**Read password from an environment variable:**

```sh
SSHPASS=hunter2 sshpass-rs -e ssh user@host
```

**Store a password in the Keychain once:**

```sh
sshpass-rs --store user@host
# Prompts: Enter password for user@host:
```

**Use a stored Keychain password (auto-detect key):**

```sh
sshpass-rs -k ssh user@host
# Key auto-derived as "user@host"
```

**Use a stored Keychain password with an explicit key:**

```sh
sshpass-rs --key myserver ssh -l user host
```

**Delete a stored password:**

```sh
sshpass-rs --delete user@host
```

**List all stored passwords:**

```sh
sshpass-rs --list
```

**Use with scp:**

```sh
sshpass-rs -k scp user@host:/remote/file ./local/
```

**Use with rsync:**

```sh
sshpass-rs -k rsync -avz user@host:/data/ ./backup/
```

**Custom prompt pattern (for non-English systems):**

```sh
sshpass-rs -p hunter2 -P "Passwort:" ssh user@host
```

### Recommended workflow

Store the password once, then use `-k` everywhere:

```sh
# One-time setup
sshpass-rs --store user@host

# Daily use
sshpass-rs -k ssh user@host
sshpass-rs -k scp user@host:/etc/config ./
sshpass-rs -k rsync -avz user@host:/data/ ./backup/
```

### Key auto-detection

When you use `-k` without `--key`, sshpass-rs inspects the wrapped command to derive a `user@host` key. It recognizes two patterns:

- `ssh user@host ...` - the first non-flag argument containing `@`
- `ssh -l user host ...` - the `-l` flag followed by a hostname

If neither pattern matches, sshpass-rs prompts you interactively for the password instead of failing.

### Default mode (no password flag)

If you don't pass any password source flag, sshpass-rs reads the password from stdin before spawning the command. This matches the original sshpass behavior.

```sh
echo "hunter2" | sshpass-rs ssh user@host
```

### Stdin forwarding

After the password is sent, stdin is forwarded to the PTY. Interactive SSH sessions work normally: you can run commands, use a shell, and type input as usual.

## Exit codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | Success | Command completed successfully |
| 1 | InvalidArguments | Missing required argument (e.g., no wrapped command) |
| 2 | ConflictingArguments | Multiple password sources specified, or bad flag syntax |
| 3 | RuntimeError | PTY creation failed, child spawn failed, Keychain access error, or I/O error |
| 4 | ParseError | Could not parse output from the child process |
| 5 | IncorrectPassword | SSH rejected the password |
| 6 | HostKeyUnknown | Host key not in known_hosts |
| 7 | HostKeyChanged | Host key changed since last connection |

## Compatibility

### Same as original sshpass

- `-p`, `-f`, `-d`, `-e` password sources work identically
- `-P` prompt pattern matching works identically
- `-v` verbose flag works identically
- Exit codes 0-7 match the original sshpass specification
- Works with any command that reads a password from a PTY

### New in sshpass-rs

- `-k` / `--key`: Keychain-backed password lookup
- `--store`: Securely store a password in the macOS Keychain
- `--delete`: Remove a stored password
- `--list`: List all managed Keychain entries
- Auto-detection of `user@host` key from the wrapped SSH command
- Interactive fallback prompt when a Keychain key is missing
- Passwords held as `SecretString` (zeroized on drop)
- `SSHPASS_RS_BACKEND`: 1Password backend support via `op` CLI

## Security

| Property | Original sshpass | sshpass-rs |
|----------|-----------------|------------|
| Password in process args (`-p`) | Visible in `ps` | Visible in `ps` (same) |
| Password in file (`-f`) | Plaintext file | Plaintext file (same) |
| Password in env (`-e`) | Cleared before exec | Cleared before exec |
| Keychain storage | Not supported | macOS Keychain (encrypted at rest) |
| 1Password storage | Not supported | `op` CLI (encrypted vault, biometric unlock) |
| In-memory handling | Plain string | `SecretString` (zeroized on drop) |

The `-p` flag is the least secure option on any platform because the password appears in the process argument list. For scripts and automation, prefer `-k` with a Keychain-stored password.

## Works with

Any command that prompts for a password on a PTY:

- `ssh`
- `scp`
- `sftp`
- `rsync` (via `--rsh`)
- Any other PTY-based tool that reads a password interactively

## Limitations

- macOS only (v1). The Keychain integration uses the Apple native backend. Linux support is not included in this release.
- The 1Password backend requires the `op` CLI to be installed separately. See [1password.com/downloads/command-line](https://1password.com/downloads/command-line/).
- No config file. All options are passed on the command line.
- No async runtime. The PTY loop is synchronous.

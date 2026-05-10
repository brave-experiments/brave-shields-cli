# brave-shields-cli

CLI tool to read and write Brave Shields settings.

## Supported platforms

- **macOS** and **Linux** are supported with automatic profile detection.
- **Windows** is not supported. You can use `--brave-dir <path>` to point at a Brave data directory manually as a workaround.

## Install

```bash
cargo install --path .
```

## Quick start

Disable Shields for example.com on Brave Release:

```bash
brave-shields-cli set example.com shields off
```

Disable Shields globally (all sites):

```bash
brave-shields-cli set "https://*,*" shields off --pattern
```

To target a specific channel (e.g. Nightly):

```bash
brave-shields-cli --channel nightly set example.com shields off
```

## Usage

```bash
brave-shields-cli <command> [options]
```

For more info, see:
```bash
brave-shields-cli --help
```

### Commands

#### get

Show shield settings for a domain.

```bash
brave-shields-cli get example.com
brave-shields-cli get example.com --format table
```

#### set

Set a shield setting for a domain.

```bash
brave-shields-cli set example.com shields off
brave-shields-cli set example.com ads aggressive
brave-shields-cli set example.com fingerprinting disabled
brave-shields-cli set example.com https-upgrade strict
brave-shields-cli set example.com scripts block
```

#### list

List all per-site shield overrides.

```bash
brave-shields-cli list
brave-shields-cli list --format table
```

#### reset

Remove shield overrides for a domain. Omit the setting name to reset all.

```bash
brave-shields-cli reset example.com
brave-shields-cli reset example.com fingerprinting
```

#### filters

Manage custom adblock filter rules. These are stored globally in `Local State` (not per-profile), matching what you see at `brave://adblock` under "Custom filters".

```bash
brave-shields-cli filters list
brave-shields-cli filters add "||example.com^"
brave-shields-cli filters add "example.com##h1"
brave-shields-cli filters remove "||example.com^"
brave-shields-cli filters clear
brave-shields-cli filters load rules.txt
brave-shields-cli filters bisect rules.txt
```

The `bisect` command performs an interactive binary search to find which filter rule in a file is causing a site issue. It loads all rules, then iteratively exceptions out half the candidates with `@@` prefixes, asking at each step whether the issue persists. A working file (`bisect-filters.txt`) and state file (`bisect-state.json`) are written to the current directory. Original filters are restored when bisect completes. If interrupted or quit mid-session, the state is preserved and can be resumed with `--resume bisect-state.json`.

#### scriptlets

Manage custom scriptlets (user-defined JavaScript for adblock injection). These are stored in a LevelDB database at `Default/AdBlock Custom Resources/` and are shared across profiles.

```bash
brave-shields-cli scriptlets list
brave-shields-cli scriptlets get user-my-script.js
brave-shields-cli scriptlets add user-my-script.js script.js
brave-shields-cli scriptlets remove user-my-script.js
```

Scriptlet names must start with `user-` and end with `.js`. Once added, reference them in custom filters with `example.com##+js(user-my-script.js)`.

#### profiles

List available browser profiles.

```bash
brave-shields-cli profiles
brave-shields-cli profiles --format table
```

### Global options

| Flag | Description |
|------|-------------|
| `--channel <channel>` | Brave channel: `release`, `beta`, `nightly`, `dev` (default: release) |
| `--profile <name>` | Profile display name or directory name (default: last used) |
| `--brave-dir <path>` | Override Brave data directory (takes precedence over `--channel`) |
| `--format json\|table` | Output format (default: json) |

The channel can also be set via the `BRAVE_SHIELDS_CLI_CHANNEL` environment variable. The `--channel` flag takes precedence over the env var.

### Raw match patterns

By default, the `get`, `set`, and `reset` commands treat the domain argument as a domain name and convert it to a Chromium content settings pattern (`domain.com,*`). Pass `--pattern` to use a raw match pattern instead. This is useful for targeting the global default or other non-standard patterns:

```bash
# Disable shields globally
brave-shields-cli set "https://*,*" shields off --pattern

# Check global shield settings
brave-shields-cli get "https://*,*" --pattern

# Reset global override
brave-shields-cli reset "https://*,*" --pattern
```

## Settings reference

| Setting | Valid values | Notes |
|---------|-------------|-------|
| `shields` | `on`, `off` | Master toggle |
| `ads` | `standard`, `aggressive`, `disabled` | Writes to shieldsAds, trackers, and cosmeticFilteringV2 |
| `fingerprinting` | `standard`, `aggressive`, `disabled` | |
| `https-upgrade` | `standard`, `strict`, `disabled` | |
| `scripts` | `allow`, `block` | |

## How it works

The tool reads and writes entries under `profile.content_settings.exceptions` in the Brave profile's `Preferences` JSON file. Each shield setting maps to one or more keys in that object, with domain entries keyed as `"domain.com,*"`.

To edit custom filters, `brave-shields-cli` works on the `Local State` file (not per-profile).

Making changes to Shields settings requires a browser restart to be picked up.

Writes are atomic (temp file + rename).

## Tests

```bash
cargo test
```

All tests use temp directories with fixture JSON files.

# brave-shields-cli

CLI tool to read and write Brave Shields settings. The tool operates on per-website preferences by modifying the local profile `Preferences` JSON file directly.

## Supported platforms

- **macOS** and **Linux** are supported with automatic profile detection.
- **Windows** is not supported. You can use `--brave-dir <path>` to point at a Brave data directory manually as a workaround.

## Install

```bash
cargo install --path .
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

## Settings reference

| Setting | Valid values | Notes |
|---------|-------------|-------|
| `shields` | `on`, `off` | Master toggle |
| `ads` | `standard`, `aggressive`, `disabled` | Writes to shieldsAds, trackers, and cosmeticFilteringV2 |
| `fingerprinting` | `standard`, `aggressive`, `disabled` | |
| `https-upgrade` | `standard`, `strict`, `disabled` | |
| `scripts` | `allow`, `block` | |

## How it works

The tool reads and writes entries under `account_values.profile.content_settings.exceptions` in the Brave profile's `Preferences` JSON file. Each shield setting maps to one or more keys in that object, with domain entries keyed as `"domain.com,*"`.

Writes are atomic (temp file + rename). If Brave is running when you write, a warning is printed to stderr and changes take effect on next browser launch.

## Tests

```bash
cargo test
```

All tests use temp directories with fixture JSON files; no real Brave profile is needed.

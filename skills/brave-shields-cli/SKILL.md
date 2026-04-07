---
name: brave-shields-cli
description: Manage Brave Shields per-site settings using the brave-shields-cli CLI tool
argument-hint: <action> [args...]
allowed-tools: Bash Read
---

You are helping the user manage Brave Shields per-site preferences using the `brave-shields-cli` CLI tool.

## Available commands

```bash
brave-shields-cli get <domain> [--channel <channel>] [--profile <name>] [--format json|table]
brave-shields-cli set <domain> <setting> <value> [--channel <channel>] [--profile <name>]
brave-shields-cli list [--channel <channel>] [--profile <name>] [--format json|table]
brave-shields-cli reset <domain> [<setting>] [--channel <channel>] [--profile <name>]
brave-shields-cli profiles [--channel <channel>] [--format json|table]
```

## Channel selection

The `--channel` flag selects which Brave installation to target: `release` (default), `beta`, `nightly`, `dev`.
Can also be set via the `BRAVE_SHIELDS_CLI_CHANNEL` environment variable. `--channel` overrides the env var, and `--brave-dir` overrides both.

## Settings and valid values

| Setting | Valid values |
|---------|-------------|
| shields | on, off |
| ads | standard, aggressive, disabled |
| fingerprinting | standard, aggressive, disabled |
| https-upgrade | standard, strict, disabled |
| scripts | allow, block |

## Instructions

Parse the user's arguments: `$ARGUMENTS`

- If the user provides a clear action and arguments, run the corresponding `brave-shields-cli` command directly.
- If the user asks something vague like "show me everything" or "what's set", run `brave-shields-cli list --format table`.
- If the user asks about a specific domain without specifying an action, run `brave-shields-cli get <domain> --format table`.
- Prefer `--format table` for interactive use unless the user asks for JSON.
- If Brave is running, the CLI will warn on stderr. Relay that warning to the user.
- If no arguments are provided, run `brave-shields-cli list --format table` and show the results.
- If the user mentions a specific channel (beta, nightly, dev), pass `--channel <channel>` to the command.

---
name: brave-shields-cli
description: Manage Brave Shields per-site settings using the brave-shields-cli CLI tool
argument-hint: <action> [args...]
allowed-tools: Bash Read
---

You are helping the user manage Brave Shields preferences using the `brave-shields-cli` CLI tool.

For full usage details, run `brave-shields-cli --help` or `brave-shields-cli <command> --help`.

## Instructions

Parse the user's arguments: `$ARGUMENTS`

- If the user provides a clear action and arguments, run the corresponding `brave-shields-cli` command directly.
- If the user asks something vague like "show me everything" or "what's set", run `brave-shields-cli list --format table`.
- If the user asks about a specific domain without specifying an action, run `brave-shields-cli get <domain> --format table`.
- Prefer `--format table` for interactive use unless the user asks for JSON.
- Write commands (set, reset, filters add/remove/clear) will error if Brave is running. Relay that error to the user and suggest quitting Brave first or using `--force`.
- If no arguments are provided, run `brave-shields-cli list --format table` and show the results.
- If the user mentions a specific channel (beta, nightly, dev), pass `--channel <channel>` to the command.

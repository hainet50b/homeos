# homeos — Developer Spec

This document captures homeos's _internal_ spec — the things a developer (or an AI agent implementing a task) needs to know but a user does not. End users read `README.md` instead; product intent and the tasks ledger live in `PRD.md`; coding style lives in `CONVENTIONS.md`.

## Data Model

### homeos.yml

```yaml
packages:
  neovim:
    script_aliases: { update: install }
    enabled: false
```

### state.yml

Tracks which packages are installed on this machine. Machine-specific, excluded from version control.

```yaml
installed:
  - neovim
  - zed
```

- `package install` adds packages to `installed`
- `package uninstall` removes packages from `installed`

## Action resolution

Scripts are resolved by convention based on the OS and executed via `std::process::Command`:

- Linux / macOS: `install.sh`, `update.sh`, `uninstall.sh` (run with `sh`)
- Windows: `install.ps1`, `update.ps1`, `uninstall.ps1` (run with `pwsh`)

`script_aliases` aliases an action to another (e.g., `{ update: install }` runs the install script for update).

## Confirmation prompt

Before executing install / update / uninstall, display the plan and prompt for confirmation (`Proceed? [y/N]`). Disabled packages are skipped with a message (e.g., `Skipping neovim (disabled)`).

## Testing script execution

Tests create temporary scripts that output a known marker string to stdout. Capture `Command` output and assert on the marker to verify execution without side effects.

## Directory structure (created by `homeos init`)

Base directory is resolved by the `dirs` crate (`data_local_dir()`), e.g., `~/.local/share/homeos` on Linux, `%LOCALAPPDATA%/homeos` on Windows.

```
<data_dir>/homeos/
├── .gitignore
├── homeos.yml
├── state.yml
├── packages/
└── plugins/
```

# PRD: homeos

## Goal

Build a CLI tool that manages application install scripts and configurations across environments.
See `README.md` for the full specification of commands, directory structure, and configuration format.

## Tech Stack

- Rust (latest stable)
- clap (CLI argument parsing)
- serde / yaml_serde (configuration parsing)
- dirs (OS-appropriate data directory resolution)

## Data Model

### homeos.yml

```yaml
packages:
  neovim:
    actions_overrides: { update: install }
    enabled: false
```

### Action resolution

Scripts are resolved by convention based on the OS:

- Linux / macOS: `install.sh`, `update.sh`, `uninstall.sh`
- Windows: `install.ps1`, `update.ps1`, `uninstall.ps1`

`actions_overrides` aliases an action to another (e.g., `{ update: install }` runs the install script for update).

### Directory structure (created by `homeos init`)

Base directory is resolved by the `dirs` crate (`data_dir()`), e.g., `~/.local/share/homeos` on Linux, `%LOCALAPPDATA%/homeos` on Windows.

```
<data_dir>/homeos/
└── repos/
    └── default/
        ├── homeos.yml
        └── packages/
```

## Tasks

- [x] Scaffold Cargo project with clap CLI skeleton (`homeos --help` works). Base directory must be injectable so tests use a `tempdir` instead of the real data directory.
- [x] Implement `homeos.yml` parsing with serde (packages with `actions_overrides` and `enabled`)
- [x] Implement `homeos init` — create directory structure and empty `homeos.yml`
- [x] Migrate from `serde_yaml` to `yaml_serde` and update all existing code and tests
- [x] Refactor all existing unit tests to follow the 3A pattern (Arrange / Act / Assert)
- [ ] Implement `homeos cd` — launch a shell in the default repository directory
- [ ] Implement `homeos package list` — list all packages from `homeos.yml`
- [ ] Implement `homeos package add <pkg>` — create package directory and add entry to `homeos.yml`
- [ ] Implement `homeos package remove <pkg>` — remove package entry from `homeos.yml`
- [ ] Implement `homeos package install [pkg]` — execute install action scripts
- [ ] Implement `homeos package update [pkg]` — execute update action scripts
- [ ] Implement `homeos package uninstall [pkg]` — execute uninstall action scripts

## Post Tasks

- [ ] Verify full workflow: `init` → `package add` → write install script → `package install`

## Completion Criteria

All tasks and post tasks are checked off and `cargo test` passes with no failures.

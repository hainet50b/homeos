# Progress

## Task: Scaffold Cargo project with clap CLI skeleton

**Timestamp:**

2026-04-03T00:00:00Z

**Why this task:**

First task in dependency order — all other tasks depend on the CLI skeleton and project structure existing.

**What was done:**

Initialized a Cargo project with clap (derive) and dirs dependencies. Created a CLI skeleton with all subcommands defined (init, cd, package list/add/remove/install/update/uninstall). Implemented a `Context` struct with injectable base directory — accepts an optional `--base-dir` override (hidden flag), defaulting to OS data directory via `dirs::data_dir()`. Added unit tests verifying path resolution with both custom and default base directories.

**What was changed:**

- Cargo.toml (new — project manifest with clap and dirs dependencies)
- src/main.rs (new — CLI definition with all subcommands)
- src/context.rs (new — Context struct with injectable base directory and tests)

**Remarks:**

- The `--base-dir` flag is hidden from `--help` output since it's only intended for testing.
- `packages_dir()` and `config_path()` methods are defined on Context but currently unused — they'll be used by subsequent tasks.

## Task: Implement `homeos.yml` parsing with serde

**Timestamp:**

2026-04-03T12:00:00Z

**Why this task:**

Second task in dependency order — config parsing is required by nearly all subsequent commands (init, package list/add/remove, etc.).

**What was done:**

Created a `config` module with `Config` and `PackageConfig` structs using serde derive. `Config` holds a `BTreeMap<String, PackageConfig>` for deterministic ordering. `PackageConfig` has `actions_overrides` (BTreeMap<String, String>) and `enabled` (bool, defaults to true). Added `load()` and `save()` methods for file I/O. Serialization skips default values (`enabled: true` and empty `actions_overrides`) for clean YAML output. Added 7 tests covering full parsing, empty/minimal configs, defaults, file I/O round-trip, and error cases.

**What was changed:**

- Cargo.toml (added serde, serde_yaml dependencies; added tempfile dev-dependency)
- src/config.rs (new — Config and PackageConfig structs with serde, load/save, tests)
- src/main.rs (added `mod config` declaration)
- prd.md (checked off task)

**Remarks:**

- Used `BTreeMap` instead of `HashMap` for deterministic serialization order in YAML output.
- `serde_yaml` 0.9 shows a deprecation warning during compilation — it still works fine but a future migration to a maintained YAML crate may be needed.
- `skip_serializing_if` is used to keep generated YAML clean by omitting default values.

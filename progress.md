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

## Task: Implement `homeos init`

**Timestamp:**

2026-04-03T14:00:00Z

**Why this task:**

Next in dependency order — `init` creates the directory structure and config file that all subsequent commands depend on.

**What was done:**

Created a `commands` module with an `init` submodule. The `init::run` function creates the full directory structure (`repos/default/packages/`) and an empty `homeos.yml` with `Config::default()`. If `homeos.yml` already exists, it prints a message and returns without overwriting (idempotent). Wired the command into `main.rs`. Added 3 tests: structure creation, idempotency (doesn't overwrite existing config), and correct directory paths.

**What was changed:**

- src/commands.rs (new — module declaration for commands)
- src/commands/init.rs (new — init command implementation with tests)
- src/main.rs (added `mod commands`, wired init command to `commands::init::run`)
- prd.md (checked off task)

**Remarks:**

- Init is idempotent: re-running on an already-initialized directory is a no-op that preserves existing config.
- The `commands` module structure is set up to accommodate future command implementations (cd, package subcommands).

## Task: Migrate from serde_yaml to yaml_serde

**Timestamp:**

2026-04-03T16:00:00Z

**Why this task:**

Next unchecked task in dependency order. serde_yaml 0.9 is deprecated; migrating to yaml_serde removes the deprecation warning and uses a maintained crate.

**What was done:**

Replaced the `serde_yaml` dependency with `yaml_serde = "0.10"` in Cargo.toml. Updated all `serde_yaml::` references in `src/config.rs` (both production code and tests) to `yaml_serde::`. The API is compatible — `from_str` and `to_string` work identically.

**What was changed:**

- Cargo.toml (replaced serde_yaml 0.9 with yaml_serde 0.10)
- src/config.rs (replaced all serde_yaml:: references with yaml_serde::)
- prd.md (checked off task)

**Remarks:**

- yaml_serde 0.10 is API-compatible with serde_yaml 0.9 for the functions used (from_str, to_string), so the migration was a straightforward find-and-replace.
- All 13 existing tests pass without modification beyond the crate name change.

## Task: Refactor all existing unit tests to follow the 3A pattern

**Timestamp:**

2026-04-03T18:00:00Z

**Why this task:**

Next unchecked task in dependency order. Establishing the 3A test convention before implementing remaining commands ensures all future tests follow a consistent pattern.

**What was done:**

Refactored all 13 existing unit tests across 3 files to follow the Arrange / Act / Assert pattern:

- **context.rs (2 tests):** Added a `fixture()` helper that wraps `Context::new`. Separated test setup (Arrange), SUT construction (Act), and assertions (Assert) with comments.
- **config.rs (8 tests):** Added `fixture()` (parses YAML string into Config) and `fixture_file()` (writes content to a temp file) helpers. Restructured all tests with clear 3A sections, naming the primary object under test `sut`.
- **commands/init.rs (3 tests):** Replaced `test_context()` with a `fixture()` that returns both TempDir and Context. Added 3A comments to all tests.

**What was changed:**

- src/context.rs (refactored tests)
- src/config.rs (refactored tests)
- src/commands/init.rs (refactored tests)
- prd.md (checked off task)

**Remarks:**

- Used `sut` as the variable name for the System Under Test to make the pattern explicit and consistent.
- Fixture functions encapsulate common setup, reducing duplication while keeping each test's Arrange section focused on test-specific inputs.
- All 13 tests continue to pass with no behavioral changes.

## Task: Implement `homeos cd`

**Timestamp:**

2026-04-03T20:00:00Z

**Why this task:**

Next unchecked task in dependency order. The `cd` command is independent of the package subcommands and is a straightforward command to implement.

**What was done:**

Created a `commands::cd` module with two functions: `resolve_target` (validates the default repo directory exists and returns its path) and `run` (resolves the target then spawns a subshell in that directory using `$SHELL`). The separation allows the directory resolution logic to be unit-tested without spawning actual shell processes. Added 3 tests covering: correct path returned, directory existence validation, and error when not initialized.

**What was changed:**

- src/commands/cd.rs (new — cd command implementation with tests)
- src/commands.rs (added `pub mod cd`)
- src/main.rs (wired cd command to `commands::cd::run`)
- prd.md (checked off task)

**Remarks:**

- The `run` function spawns a subshell using `$SHELL` (falling back to `/bin/sh`), since a CLI tool cannot change the parent shell's working directory directly.
- `resolve_target` is separated from `run` to keep the testable logic isolated from the shell-spawning side effect.
- All 19 tests pass (16 existing + 3 new).

## Task: Implement `homeos package list`

**Timestamp:**

2026-04-03T22:00:00Z

**Why this task:**

Next unchecked task in dependency order. The `package list` command is a prerequisite for understanding and testing subsequent package management commands (add, remove, install, etc.).

**What was done:**

Created a `commands::package` module with a `list` function that loads `homeos.yml` and prints each package name. Disabled packages are annotated with `(disabled)`. Wired the command into `main.rs`. Added 4 tests: listing multiple packages, empty packages, error when not initialized, and correct formatting of enabled/disabled packages.

**What was changed:**

- src/commands/package.rs (new — package list implementation with tests)
- src/commands.rs (added `pub mod package`)
- src/main.rs (wired list command to `commands::package::list`)
- prd.md (checked off task)

**Remarks:**

- Uses `BTreeMap` iteration from `Config`, so packages are listed in alphabetical order.
- Disabled packages show as `name (disabled)` matching the convention from the README spec.
- All 23 tests pass (19 existing + 4 new).

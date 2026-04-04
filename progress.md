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

## Task: Implement `homeos package add <pkg>`

**Timestamp:**

2026-04-03T23:00:00Z

**Why this task:**

Next unchecked task in dependency order. `package add` is a prerequisite for testing subsequent package operations (install, update, uninstall) since they need packages to exist.

**What was done:**

Created an `add` function in `commands::package` that loads `homeos.yml`, checks for duplicate package names, inserts a default `PackageConfig` entry, saves the config, and creates the package directory under `packages/<pkg>/`. Wired the command into `main.rs`. Added 4 tests: successful creation (directory + config entry), duplicate package error, error when not initialized, and preservation of existing packages.

**What was changed:**

- src/commands/package.rs (added `add` function and 4 tests)
- src/main.rs (wired `PackageCommands::Add` to `commands::package::add`)
- prd.md (checked off task)

**Remarks:**

- The function errors early if the package already exists in `homeos.yml`, preventing accidental overwrites.
- `create_dir_all` is used for the package directory so it works even if the `packages/` parent doesn't exist yet.
- All 27 tests pass (23 existing + 4 new).

## Task: Implement `homeos package remove <pkg>`

**Timestamp:**

2026-04-03T23:30:00Z

**Why this task:**

Next unchecked task in dependency order. `package remove` is the counterpart to `package add` and completes the basic package CRUD operations before moving to action execution commands.

**What was done:**

Created a `remove` function in `commands::package` that loads `homeos.yml`, checks the package exists, removes the entry from the config, and saves. Per the README spec, only the config entry is removed (not the package directory). Wired the command into `main.rs`. Added 4 tests: successful removal preserving other packages, error on nonexistent package, error when not initialized, and removing the last package leaves an empty packages map.

**What was changed:**

- src/commands/package.rs (added `remove` function and 4 tests)
- src/main.rs (wired `PackageCommands::Remove` to `commands::package::remove`)
- prd.md (checked off task)

**Remarks:**

- The spec says "Remove the package entry from `homeos.yml`" — it does not mention removing the package directory, so the implementation only modifies the config file.
- All 31 tests pass (27 existing + 4 new).

## Task: Enhance `homeos package add` to generate skeleton action scripts

**Timestamp:**

2026-04-03T12:00:00Z

**Why this task:**

Next unchecked task in dependency order. Skeleton scripts are a natural extension of `package add` and are needed before implementing install/update/uninstall execution.

**What was done:**

Enhanced the `add` function to generate skeleton action scripts (install, update, uninstall) with OS-appropriate extensions (.sh on Linux/macOS, .ps1 on Windows). Each script contains a shebang line and a comment noting it was generated by homeos and needs to be filled in with the appropriate logic. Extracted `skeleton_scripts()` to determine action names and file extensions, and `skeleton_script_content()` to generate the script body. Added 2 new tests: one verifying all three scripts are created, and one verifying the script content contains the expected comment and package name.

**What was changed:**

- src/commands/package.rs (added skeleton script generation in `add`, added `skeleton_scripts()` and `skeleton_script_content()` helpers, added 2 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Uses `cfg!(windows)` at compile time to determine the script extension, matching the PRD's OS-appropriate extension requirement.
- All 33 tests pass (31 existing + 2 new).

## Task: Implement `homeos package enable <pkg>`

**Timestamp:**

2026-04-03T12:30:00Z

**Why this task:**

Next unchecked task in dependency order. Enable/disable are prerequisites for the confirmation prompt task which checks enabled status.

**What was done:**

Added an `enable` function to `commands::package` that loads the config, finds the package, sets `enabled: true`, and saves. If the package is already enabled, it prints a message and returns without writing. Added `Enable` variant to `PackageCommands` enum and wired it in `main.rs`. Added 5 tests covering: enabling a disabled package, already-enabled noop, package not found error, not-initialized error, and preservation of other fields (actions_overrides).

**What was changed:**

- src/commands/package.rs (added `enable` function and 5 tests)
- src/main.rs (added `Enable` variant to `PackageCommands`, wired to `commands::package::enable`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `enable` function removes `enabled: false` by setting it to `true`, which then gets skipped during serialization (`skip_serializing_if = "is_true"`), resulting in clean YAML output.
- All 38 tests pass (33 existing + 5 new).

## Task: Implement `homeos package disable <pkg>`

**Timestamp:**

2026-04-03T13:00:00Z

**Why this task:**

Next unchecked task in dependency order. Disable is the counterpart to enable and is a prerequisite for the confirmation prompt task which checks enabled status.

**What was done:**

Added a `disable` function to `commands::package` that loads the config, finds the package, sets `enabled: false`, and saves. If the package is already disabled, it prints a message and returns without writing. Added `Disable` variant to `PackageCommands` enum and wired it in `main.rs`. Added 5 tests covering: disabling an enabled package, already-disabled noop, package not found error, not-initialized error, and preservation of other fields (actions_overrides).

**What was changed:**

- src/commands/package.rs (added `disable` function and 5 tests)
- src/main.rs (added `Disable` variant to `PackageCommands`, wired to `commands::package::disable`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `disable` function is the exact mirror of `enable`, setting `enabled: false` which gets serialized to YAML (unlike `true` which is skipped as a default).
- All 43 tests pass (38 existing + 5 new).

## Task: Implement confirmation prompt

**Timestamp:**

2026-04-03T13:30:00Z

**Why this task:**

Next unchecked task in dependency order. The confirmation prompt is a prerequisite for install/update/uninstall commands which need to show a plan and get user confirmation before executing.

**What was done:**

Created a `confirm` module with three main components: `Plan` struct (builds a plan separating enabled/disabled packages, formats it for display), `prompt_confirm` function (reads y/Y from a `BufRead` and writes the prompt to a `Write`), and `confirm_plan` function (combines plan display with confirmation prompt). The plan logic is fully separated from I/O by accepting generic `BufRead`/`Write` traits, making it unit-testable with `Cursor` buffers. Added 16 tests covering plan building (enabled/disabled separation, all-enabled, all-disabled, unknown package error), plan display (mixed, disabled-only, verb forms for install/update/uninstall), prompt confirmation (y, Y, n, empty, prompt text output), confirm_plan integration, and `is_empty` checks.

**What was changed:**

- src/confirm.rs (new — Plan struct, prompt_confirm, confirm_plan, 16 tests)
- src/main.rs (added `mod confirm` declaration)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Used generic `BufRead`/`Write` traits for I/O injection rather than closures or trait objects, keeping the API simple while enabling test isolation.
- The `display()` method maps action names to past-tense verbs (install -> installed, update -> updated, uninstall -> uninstalled) matching the README example format.
- All 59 tests pass (43 existing + 16 new).

## Task: Implement `homeos package install <pkg>...`

**Timestamp:**

2026-04-03T14:00:00Z

**Why this task:**

Next unchecked task in dependency order. Install is the first of the three action execution commands (install/update/uninstall) and establishes the shared `run_action` pattern.

**What was done:**

Implemented `homeos package install <pkg>...` which executes install action scripts for specified packages. Created a generic `run_action` function (reusable for update/uninstall) that: loads config, builds a confirmation plan, prompts the user, resolves scripts (respecting `actions_overrides`), and executes them via `sh` (or `powershell` on Windows). Added `resolve_script_name` (maps action to script filename with override support), `execute_script` (runs a script via OS shell and captures output), and a thin `install` wrapper. Changed the CLI `Install` variant from `Option<String>` to `Vec<String>` with `required = true` to accept one or more packages (also updated `Update` and `Uninstall` variants for consistency). I/O is injectable via `BufRead`/`Write` traits for full testability. Added 9 tests covering: script name resolution (default and with override), script execution with marker capture, full install flow, disabled package skipping, abort on no confirmation, missing script error, action override execution, and multiple package execution.

**What was changed:**

- src/commands/package.rs (added `install`, `run_action`, `resolve_script_name`, `execute_script`, and 9 tests)
- src/main.rs (changed Install/Update/Uninstall to `Vec<String>`, wired install command)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `run_action` function is designed to be reused by `update` and `uninstall` — they only need thin wrappers passing the action name.
- Changed all three action CLI variants (Install/Update/Uninstall) from `Option<String>` to `Vec<String>` at once for consistency, since they share the same `<pkg>...` interface.
- All 68 tests pass (59 existing + 9 new).

## Task: Implement `homeos package update <pkg>...`

**Timestamp:**

2026-04-03T14:30:00Z

**Why this task:**

Next unchecked task in dependency order. Update is the second of the three action execution commands and reuses the `run_action` infrastructure established by install.

**What was done:**

Added an `update` function in `commands::package` as a thin wrapper around `run_action` with action name "update". Wired the `PackageCommands::Update` variant in `main.rs` to call `commands::package::update` (replacing the placeholder `println!`). Added 3 tests covering: successful update execution, disabled package skipping for update, and abort on no confirmation for update.

**What was changed:**

- src/commands/package.rs (added `update` function and 3 tests)
- src/main.rs (wired `PackageCommands::Update` to `commands::package::update`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The implementation is a one-line wrapper since `run_action` was designed to be generic across install/update/uninstall.
- The existing `test_run_action_respects_action_overrides` test already covers the update action with overrides, so the new tests focus on the update-specific output formatting.
- All 71 tests pass (68 existing + 3 new).

## Task: Implement `homeos package uninstall <pkg>...`

**Timestamp:**

2026-04-03T15:00:00Z

**Why this task:**

Last remaining unchecked task in the PRD. All dependencies (run_action infrastructure, confirmation prompt) were already in place from previous tasks.

**What was done:**

Added an `uninstall` function in `commands::package` as a thin wrapper around `run_action` with action name "uninstall". Wired the `PackageCommands::Uninstall` variant in `main.rs` to call `commands::package::uninstall` (replacing the placeholder `println!`). Added 3 tests covering: successful uninstall execution, disabled package skipping for uninstall, and abort on no confirmation for uninstall.

**What was changed:**

- src/commands/package.rs (added `uninstall` function and 3 tests)
- src/main.rs (wired `PackageCommands::Uninstall` to `commands::package::uninstall`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The implementation is a one-line wrapper since `run_action` was designed to be generic across install/update/uninstall.
- The "Uninstalling" verb was already handled in `run_action`'s match arm, so no changes were needed to the shared code.
- All 74 tests pass (71 existing + 3 new).

## Task: Implement `homeos package cat <pkg>`

**Timestamp:**

2026-04-03T15:30:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All dependencies were already in place.

**What was done:**

Added a `cat` command that displays all action scripts (install, update, uninstall) for a given package with `=== <filename> ===` headers. If a script file does not exist, `(not found)` is shown instead of its content. Implemented as a `cat` public function (delegates to stdout) and a `cat_to` internal function accepting a generic `Write` for testability. Added `Cat` variant to `PackageCommands` and wired it in `main.rs`. Added 5 tests covering: all scripts present, mixed present/missing scripts, all scripts missing, package not found error, and not-initialized error.

**What was changed:**

- src/commands/package.rs (added `cat` and `cat_to` functions and 5 tests)
- src/main.rs (added `Cat` variant to `PackageCommands`, wired to `commands::package::cat`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Used a `cat_to` inner function with a `Write` parameter (same pattern as `run_action`) to keep output capturable in tests without relying on stdout capture.
- The output format matches the README spec exactly: `=== install.sh ===` headers with blank lines between sections.
- All 79 tests pass (74 existing + 5 new).

## Task: Implement `state.yml` parsing with serde

**Timestamp:**

2026-04-03T16:00:00Z

**Why this task:**

First unchecked task in dependency order. `state.yml` parsing is required by subsequent tasks that record/remove installed packages.

**What was done:**

Created a `state` module with a `State` struct containing an `installed: Vec<String>` field using serde derive. Added `load()` and `save()` methods for file I/O (same pattern as `Config`). Added `state_path()` method to `Context` returning `<default_repo_dir>/state.yml`. Added 7 tests for State (parsing, empty, defaults, file I/O round-trip, error case, default state) and 1 test for `state_path()`.

**What was changed:**

- src/state.rs (new — State struct with serde, load/save, 7 tests)
- src/main.rs (added `mod state` declaration)
- src/context.rs (added `state_path()` method and test)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Followed the exact same patterns as `config.rs` for consistency (load/save methods, fixture helpers, 3A test pattern).
- All 87 tests pass (79 existing + 7 new state tests + 1 new context test).

## Task: Enhance `homeos init` to generate `.gitignore` that excludes `state.yml`

**Timestamp:**

2026-04-03T16:30:00Z

**Why this task:**

Next unchecked task in dependency order. The `.gitignore` is part of the directory structure defined in the PRD and ensures `state.yml` (machine-specific state) is excluded from version control.

**What was done:**

Added a `gitignore_path()` method to `Context` returning `<default_repo_dir>/.gitignore`. Enhanced `init::run` to write a `.gitignore` file containing `state.yml` after creating the config. The idempotency check (early return when `homeos.yml` exists) ensures an existing `.gitignore` is not overwritten on re-init. Added 3 tests: `gitignore_path()` resolution, `.gitignore` creation with correct content, and idempotency preserving a modified `.gitignore`.

**What was changed:**

- src/context.rs (added `gitignore_path()` method and test)
- src/commands/init.rs (added `.gitignore` creation in `run`, added 2 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `.gitignore` is created after `homeos.yml` so it's covered by the existing idempotency guard — if init was already run, neither file is touched.
- All 90 tests pass (87 existing + 1 new context test + 2 new init tests).

## Task: Enhance `homeos package install` to record installed packages in `state.yml`

**Timestamp:**

2026-04-03T17:00:00Z

**Why this task:**

Next unchecked task in dependency order. Depends on `state.yml` parsing (completed) and `package install` (completed).

**What was done:**

Enhanced `run_action` to record installed packages in `state.yml` after successful install execution. When the action is "install", the function loads `state.yml` (or creates a default `State` if the file doesn't exist), appends newly installed packages (avoiding duplicates), and saves. Added `use crate::state::State` import. Added 5 tests: recording a package in state, creating state file when missing, appending to existing state, deduplication when already in state, and verifying update does not modify state.

**What was changed:**

- src/commands/package.rs (added state recording logic in `run_action`, added `State` import, added 5 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The state recording only happens for the "install" action, not for "update" or "uninstall" (uninstall will be handled in the next task).
- Deduplication uses `Vec::contains` which is O(n) but sufficient for the expected small number of packages.
- All 95 tests pass (90 existing + 5 new).

## Task: Enhance `homeos package uninstall` to remove uninstalled packages from `state.yml`

**Timestamp:**

2026-04-03T17:30:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All dependencies (state.yml parsing, package uninstall execution) were already in place.

**What was done:**

Enhanced `run_action` to remove uninstalled packages from `state.yml` after successful uninstall execution. When the action is "uninstall" and `state.yml` exists, the function loads the state, removes the uninstalled packages using `Vec::retain`, and saves. If `state.yml` does not exist, the uninstall proceeds without state modification (no-op). Added 4 tests covering: removing a package from state, no-op when state file is missing, removing multiple packages from state, and ignoring packages not present in state.

**What was changed:**

- src/commands/package.rs (added state removal logic in `run_action` for "uninstall" action, added 4 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The state removal only happens when `state.yml` exists — unlike install which creates it if missing, uninstall has nothing to do if there's no state file.
- All 99 tests pass (95 existing + 4 new).

## Task: Enhance `homeos package install` to skip already-installed packages

**Timestamp:**

2026-04-03T18:00:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All dependencies (state.yml parsing, package install with state recording) were already in place.

**What was done:**

Enhanced `Plan::build` to accept an `installed` slice parameter. When a package is both enabled and present in the installed list, it is classified into a new `already_installed` field instead of `enabled`. Updated `Plan::display` to show `Skipping <pkg> (already installed)` for these packages. Modified `run_action` to load `state.yml` (when action is "install") and pass the installed list to `Plan::build`. Already-installed packages are not executed. Added 3 new tests to `confirm.rs` (classifies already-installed, all already-installed, display message) and 4 new tests to `package.rs` (skip single, skip with mix, all already-installed, update ignores installed state).

**What was changed:**

- src/confirm.rs (added `already_installed` field to `Plan`, updated `build` signature and logic, updated `display`, added 3 tests, updated all existing test call sites)
- src/commands/package.rs (updated `run_action` to load state for install action and pass to `Plan::build`, added 4 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `installed` parameter is passed as `&[]` for non-install actions, so update/uninstall behavior is unchanged.
- Disabled packages take precedence over already-installed in the classification logic (checked first).
- All 106 tests pass (99 existing + 3 new confirm tests + 4 new package tests).

## Task: Refactor install/update/uninstall to update state.yml per package

**Timestamp:**

2026-04-04T12:00:00Z

**Why this task:**

First unchecked task in dependency order. Per-package state updates are a prerequisite for correctness — if a later package fails, earlier successful packages must already be recorded in state.yml.

**What was done:**

Refactored `run_action` to update `state.yml` after each successful script execution instead of in bulk at the end. Extracted an `update_state_per_package` helper that handles both install (add to state) and uninstall (remove from state) per package. Changed error handling so that script failures (missing script or execution failure) are reported to the user and the loop continues to the next package. The function returns `Err("Some packages failed")` if any package failed, ensuring callers know about partial failures. Updated the existing `test_run_action_errors_on_missing_script` test to match new behavior. Added 4 new tests covering: install state recorded on partial failure (first succeeds, second missing script), install continues after script execution failure, uninstall per-package state recording, and uninstall state on partial failure.

**What was changed:**

- src/commands/package.rs (refactored `run_action` loop, added `update_state_per_package` helper, updated 1 test, added 4 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `update_state_per_package` function loads and saves state.yml on each call. This is correct for ensuring durability but involves repeated file I/O. Acceptable for the expected small number of packages.
- All 110 tests pass (106 existing + 4 new).

## Task: Enhance `homeos package uninstall` to disable uninstalled packages in `homeos.yml`

**Timestamp:**

2026-04-04T12:30:00Z

**Why this task:**

First unchecked task in dependency order. No dependencies on other unchecked tasks.

**What was done:**

Enhanced `update_state_per_package` to also disable packages in `homeos.yml` after successful uninstall execution. When the action is "uninstall" and the script succeeds, the function loads the config, sets `enabled: false` on the package (if it exists and is currently enabled), and saves. Packages that fail uninstall remain enabled. Already-disabled packages are not touched. Added 4 new tests covering: single package disable after uninstall, multiple packages disable, no disable on failure, and already-disabled package stays disabled.

**What was changed:**

- src/commands/package.rs (added config disable logic in `update_state_per_package` for "uninstall" action, added 4 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The disable logic is placed inside `update_state_per_package` alongside the state.yml update, keeping the per-package post-execution side effects co-located.
- Config is loaded and saved per package (same pattern as state.yml). This is fine for small package counts.
- All 114 tests pass (110 existing + 4 new).

## Task: Fix `homeos package add` to preserve existing scripts

**Timestamp:**

2026-04-04T14:00:00Z

**Why this task:**

Next unchecked task in dependency order. No dependencies on the remaining `--all` flag task.

**What was done:**

Fixed the `add` function to check if each skeleton script file already exists before writing it. Previously, `add` unconditionally overwrote all scripts in the package directory. Now it only generates skeleton scripts for files that are missing, preserving any user-written scripts.

**What was changed:**

- src/commands/package.rs (added existence check in script generation loop, added 3 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The fix is a single `if !path.exists()` guard around the write call — minimal and targeted.
- Added 3 tests: preserving one existing script while generating others, preserving two existing scripts, and preserving all three existing scripts.
- All 117 tests pass (114 existing + 3 new).

## Task: Add `--all` flag to `homeos package uninstall`

**Timestamp:**

2026-04-04T15:00:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All dependencies (state.yml parsing, uninstall execution, per-package state updates) were already in place.

**What was done:**

Added `--all` flag to the `Uninstall` CLI variant using clap's `required_unless_present` to make packages optional when `--all` is set. Created an `uninstall_to` function that resolves packages from `state.yml` when `--all` is true, then delegates to `run_action`. When `--all` is used with no installed packages (missing or empty `state.yml`), it shows "No packages to uninstall." The confirmation prompt displays the full list of installed packages before execution.

**What was changed:**

- src/main.rs (added `all: bool` field to `Uninstall` variant with `required_unless_present`, updated match arm)
- src/commands/package.rs (added `uninstall_to` function, updated `uninstall` signature, added 5 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Used clap's `required_unless_present = "all"` attribute so that either package names or `--all` must be provided, but not neither.
- When `--all` is set and `state.yml` is missing or empty, the resolved packages list is empty, which triggers the existing "No packages to uninstall" path in `run_action`.
- All 122 tests pass (117 existing + 5 new).

## Task: Fix `homeos package uninstall` to ignore disabled status

**Timestamp:**

2026-04-04T16:00:00Z

**Why this task:**

Only remaining unchecked task in the PRD. No dependencies on other tasks.

**What was done:**

Fixed `Plan::build` to skip the disabled check when the action is "uninstall". Previously, disabled packages were classified into the `disabled` list and skipped during uninstall — now they are classified as `enabled` and executed normally. Updated the existing `test_run_action_skips_disabled_packages_for_uninstall` test (renamed to `test_run_action_executes_disabled_packages_for_uninstall`) to verify that disabled packages are executed for uninstall. Added 2 new tests in `confirm.rs`: one verifying uninstall ignores disabled status, and one verifying install still skips disabled packages. Also fixed 2 pre-existing clippy warnings (unused `BTreeMap` import in `confirm.rs`, collapsible if in `package.rs`).

**What was changed:**

- src/confirm.rs (changed `Plan::build` to skip disabled check for uninstall, moved `BTreeMap` import to test module, added 2 tests)
- src/commands/package.rs (updated 1 test to match new behavior, fixed collapsible if clippy warning)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The fix is a single condition change: `if !pkg.enabled` → `if !pkg.enabled && action != "uninstall"` in `Plan::build`.
- All 124 tests pass (122 existing + 2 new).

## Task: Extend Plan with not_installed classification and refactor state loading

**Timestamp:**

2026-04-04T17:00:00Z

**Why this task:**

First unchecked task in dependency order. This is a prerequisite for the behavior matrix task which needs the not_installed classification to skip not-in-state packages for update/uninstall.

**What was done:**

Added `not_installed: Vec<String>` field to the `Plan` struct. Updated `Plan::display` to show "Skipping <pkg> (not installed)" messages for packages in this classification. Refactored `run_action` to load `state.yml` for all actions (install, update, uninstall) instead of only for install. Made `already_installed` classification action-specific — it only applies for the "install" action, so update/uninstall are unaffected by the installed list (preserving current behavior). Updated all `Plan` struct literals in tests to include the new `not_installed` field. Added 5 new tests: 3 in confirm.rs (not_installed empty by default, already_installed only for install, display shows not_installed) and 2 in package.rs (update and uninstall load state for plan).

**What was changed:**

- src/confirm.rs (added `not_installed` field to `Plan`, updated `build` and `display`, updated all test Plan literals, added 3 tests)
- src/commands/package.rs (refactored `run_action` to load state for all actions, added 2 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `not_installed` field is structurally in place but not yet populated by `Plan::build`. The next task (behavior matrix) will add the classification logic to populate it for update/uninstall actions.
- The `already_installed` check is now guarded by `action == "install"` to prevent in-state packages from being incorrectly skipped during update/uninstall.
- All 129 tests pass (124 existing + 3 new confirm tests + 2 new package tests).

## Task: Implement the full behavior matrix

**Timestamp:**

2026-04-04T18:00:00Z

**Why this task:**

First unchecked task in dependency order. The `not_installed` classification was structurally added in the previous task but not yet populated — this task implements the actual classification logic in `Plan::build`.

**What was done:**

Rewrote `Plan::build` classification logic to use a match on action with the full behavior matrix from README:
- **install**: disabled → skip (disabled); in state → skip (already installed); else → execute
- **update**: disabled → skip (disabled); in state → execute; not in state → skip (not installed)
- **uninstall**: in state → execute (ignores disabled); not in state → skip (not installed)

Updated 6 existing tests that were missing `state.yml` setup — update and uninstall tests now create state with the package installed, which is required by the new behavior (previously these actions didn't check state). Added 12 new behavior matrix tests in `confirm.rs` covering all 4 states × 3 actions from the README table.

**What was changed:**

- src/confirm.rs (rewrote `Plan::build` classification logic, fixed 2 existing tests, added 12 behavior matrix tests)
- src/commands/package.rs (added `State` setup to 6 existing tests for update/uninstall)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The previous implementation used a flat if/else chain that didn't fully account for the state dimension. The new match-based approach maps directly to the behavior matrix, making each action's logic explicit and independent.
- All 141 tests pass (129 existing + 12 new behavior matrix tests).

## Task: Refactor action from `&str` to an `Action` enum

**Timestamp:**

2026-04-04T19:00:00Z

**Why this task:**

First unchecked task in dependency order. No dependencies on other unchecked tasks. Eliminates stringly-typed action parameters and default/fallback branches throughout the codebase.

**What was done:**

Created an `Action` enum with `Install`, `Update`, `Uninstall` variants in `confirm.rs`. Added methods: `as_str()` (for script filenames and override lookups), `past_tense()` (for plan display), `gerund()` (for progress messages), and a `Display` impl. Changed `Plan.action` from `String` to `Action`. Updated `Plan::build`, `run_action`, `update_state_per_package`, and `resolve_script_name` signatures from `action: &str` to `action: Action`. Replaced all string match arms with exhaustive enum matches — no more default/fallback branches. Updated all call sites in `install`, `update`, `uninstall_to`, and all ~60 test call sites. Added 5 new tests for the `Action` enum covering `as_str`, `past_tense`, `gerund`, `Display`, and equality.

**What was changed:**

- src/confirm.rs (added `Action` enum with methods and `Display` impl, changed `Plan.action` to `Action`, updated `Plan::build` signature, updated all test call sites, added 5 new Action tests)
- src/commands/package.rs (updated `run_action`, `update_state_per_package`, `resolve_script_name` signatures, replaced string comparisons with enum matches, updated all test call sites)

**Remarks:**

- The `Action` enum is defined in `confirm.rs` alongside `Plan` since they are tightly coupled. Both `confirm.rs` and `commands/package.rs` use it.
- `update_state_per_package` now uses a match with an explicit `Action::Update => {}` arm instead of an `else if` chain, making it clear that update is intentionally a no-op for state.
- `fixture_with_script` still takes `action: &str` since it's used for constructing script filenames, not for the Action enum.
- All 146 tests pass (141 existing + 5 new Action enum tests).

## Task: Enhance `homeos package enable` and `homeos package disable` to accept multiple packages

**Timestamp:**

2026-04-04T21:00:00Z

**Why this task:**

Next unchecked tasks in dependency order. Enable and disable are identical in structure (same signature change, same loop pattern), so they were implemented together as a single cohesive change.

**What was done:**

Changed both `enable` and `disable` CLI subcommands from accepting a single `package: String` to `packages: Vec<String>` with `#[arg(required = true)]`. Updated handler functions to accept `&[String]` and iterate over all packages, loading/saving config once (not per package). Already-enabled/disabled packages print a message and `continue` rather than returning early. Updated CLI dispatch in `main.rs`. Updated all existing test call sites to pass `&["name".to_string()]`. Added 6 new tests: 3 for enable (multiple packages, mixed already-enabled, error on not found) and 3 for disable (same pattern).

**What was changed:**

- src/main.rs (changed Enable/Disable args from `package: String` to `packages: Vec<String>`, updated dispatch)
- src/commands/package.rs (changed `enable`/`disable` signatures to `&[String]`, added loop, updated all test call sites, added 6 new tests)
- prd.md (checked off both tasks)
- progress.md (added this entry)

**Remarks:**

- Config is loaded once and saved once per invocation, not per package — this is more efficient and ensures atomicity.
- If any package is not found, the function returns an error immediately without saving partial changes. This is consistent with how other multi-package commands (install/update/uninstall) handle unknown packages via `Plan::build`.
- All 152 tests pass (146 existing + 6 new multi-package enable/disable tests).

## Task: Enhance `homeos package remove` to reject installed packages

**Timestamp:**

2026-04-04T22:00:00Z

**Why this task:**

Next unchecked task in dependency order. No dependencies on other unchecked tasks.

**What was done:**

Enhanced `remove` to check `state.yml` before removing a package. If the package is listed in `state.yml`'s `installed` list, the function returns an error telling the user to uninstall first (with the exact command). If `state.yml` does not exist, removal proceeds normally (no packages can be installed without a state file). Added 3 new tests: rejecting an installed package (verifying config is unchanged), allowing removal of an uninstalled package when other packages are installed, and allowing removal when no state file exists.

**What was changed:**

- src/commands/package.rs (added state.yml check in `remove`, added 3 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The state check happens after the "not found" check but before the actual removal, so both error paths are clean.
- All 155 tests pass (152 existing + 3 new remove-rejection tests).

## Task: Enhance `homeos package list` to show a table with Enabled and Installed columns

**Timestamp:**

2026-04-04T23:00:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All dependencies (homeos.yml parsing, state.yml parsing) were already in place.

**What was done:**

Refactored `list` to use the `list_to` pattern (generic writer) for testability. The function now loads both `homeos.yml` and `state.yml` (gracefully handling missing state file), then outputs a formatted table with Package, Enabled, and Installed columns. Column width dynamically adjusts to the longest package name. Empty config produces no output. Rewrote 3 existing list tests to verify table output via `list_to` and added 4 new tests: enabled/disabled status, installed status with state file, missing state file defaults to "no", and table header/separator format.

**What was changed:**

- src/commands/package.rs (refactored `list` into `list`/`list_to`, added table formatting with state.yml loading, rewrote 3 existing tests, added 4 new tests, removed 1 obsolete test)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Followed the same `_to` writer pattern used by `cat_to` for consistent testability across output-producing functions.
- Fixed 2 clippy warnings about `write_literal` by inlining string literals into the format string.
- All 158 tests pass (155 existing + 4 new list tests - 1 removed obsolete test).

## Task: Split `commands/package.rs` into submodules

**Timestamp:**

2026-04-04T23:30:00Z

**Why this task:**

First unchecked task in the Post Tasks (refactoring) section. All feature tasks are complete; this is the first refactoring step with no dependencies on other unchecked tasks.

**What was done:**

Split the monolithic `commands/package.rs` (2255 lines) into three files following the PRD specification:
- `commands/package/mod.rs` — module declarations, re-exports, and thin wrappers for `install`, `update`, `uninstall` (including `uninstall_to` which resolves `--all` flag before delegating to `run_action`).
- `commands/package/registry.rs` — registry operations: `list`, `list_to`, `add`, `remove`, `enable`, `disable`, `cat`, `cat_to`, plus `skeleton_scripts` and `skeleton_script_content` helpers. Includes all registry-related tests and the `fixture` helper.
- `commands/package/action.rs` — action execution: `run_action`, `execute_script`, `resolve_script_name`, `update_state_per_package`. Includes all action-related tests with both `fixture` and `fixture_with_script` helpers.

No behavioral changes — purely a structural refactoring.

**What was changed:**

- src/commands/package.rs (deleted)
- src/commands/package/mod.rs (new — module glue and uninstall wrappers)
- src/commands/package/registry.rs (new — registry functions and tests)
- src/commands/package/action.rs (new — action functions and tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Tests that called `uninstall_to` in the action module needed to reference `crate::commands::package::uninstall_to` since it lives in `mod.rs`, not `action.rs`. Made `uninstall_to` `pub(crate)` to support this.
- `commands.rs` required no changes — `pub mod package;` resolves to `package/mod.rs` automatically.
- All 158 tests pass with no changes to test logic, only import path adjustments.

## Task: Extract `script_extension()` into `commands/package/mod.rs`

**Timestamp:**

2026-04-04T23:55:00Z

**Why this task:**

Next unchecked task in PRD. No dependencies on other unchecked tasks. Reduces code duplication across the package module.

**What was done:**

Extracted `script_extension() -> &'static str` and `shell_command() -> &'static str` into `commands/package/mod.rs` to centralize OS-specific logic. Replaced 25 scattered `cfg!(windows)` checks across `action.rs` (14 occurrences) and `registry.rs` (10 occurrences) with calls to these functions. Added 2 unit tests in `mod.rs` verifying both functions return OS-appropriate values.

**What was changed:**

- src/commands/package/mod.rs (added `script_extension()`, `shell_command()`, and tests)
- src/commands/package/action.rs (replaced all `cfg!(windows)` with `script_extension()` / `shell_command()`)
- src/commands/package/registry.rs (replaced all `cfg!(windows)` with `script_extension()`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- Also extracted `shell_command() -> &'static str` alongside `script_extension()` since `execute_script` had the same pattern for resolving the shell binary.
- Test modules import via `use crate::commands::package::script_extension;` for cleaner references instead of `super::super::`.
- All 160 tests pass (158 existing + 2 new).

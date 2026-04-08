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

## Task: Rename `confirm.rs` to `plan.rs`

**Timestamp:**

2026-04-04T24:30:00Z

**Why this task:**

Next unchecked task in PRD. No dependencies on other unchecked tasks. Pure rename to better reflect module responsibility.

**What was done:**

Renamed `src/confirm.rs` to `src/plan.rs` and updated all three import sites: `mod confirm` in `main.rs`, `use crate::confirm::Action` in `commands/package/mod.rs`, and `use crate::confirm::{confirm_plan, Action, Plan}` in `commands/package/action.rs`.

**What was changed:**

- src/confirm.rs (renamed to src/plan.rs)
- src/main.rs (mod confirm -> mod plan)
- src/commands/package/mod.rs (use crate::confirm -> use crate::plan)
- src/commands/package/action.rs (use crate::confirm -> use crate::plan)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- No new tests needed — this is a pure file rename with import path updates. All 160 existing tests pass unchanged.

## Task: Reorder functions and methods to match README command order

**Timestamp:**

2026-04-04T25:00:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All feature and refactoring tasks are complete. This is a code organization task to align source code ordering with the README command definition order.

**What was done:**

Reordered functions and tests across the package module to match the README command definition order: list, add, remove, enable, disable, cat, cd, install, update, uninstall. Specific changes:
- `commands/package/mod.rs`: Reordered `pub use` exports from alphabetical (`add, cat, disable, enable, list, remove`) to README order (`list, add, remove, enable, disable, cat`).
- `commands/package/registry.rs`: Swapped `remove` and `add` function definitions so `add` comes before `remove`. Moved `test_list_table_header_and_separator` from between add and remove test groups to the list test group. Moved `test_remove_last_package_leaves_empty_packages` from after the disable test group to the remove test group.

No behavioral changes — purely ordering adjustments.

**What was changed:**

- src/commands/package/mod.rs (reordered pub use exports)
- src/commands/package/registry.rs (reordered add/remove functions, relocated 2 misplaced tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- `main.rs` already had correct ordering for both `PackageCommands` enum variants and match arms — no changes needed.
- `action.rs` functions (run_action, execute_script, resolve_script_name, update_state_per_package) are internal helpers, not command-level functions, so their order was not adjusted.
- All 160 tests pass with no changes to test logic.

## Task: Move action functions from mod.rs to action.rs

**Timestamp:**

2026-04-04T10:00:00Z

**Why this task:**

Only remaining unchecked task in the PRD. All other Tasks and Post Tasks are complete.

**What was done:**

Moved `install`, `update`, `uninstall`, and `uninstall_to` from `commands/package/mod.rs` to `commands/package/action.rs`. Updated `mod.rs` to re-export `install`, `update`, and `uninstall` via `pub use action::{install, update, uninstall}`. Removed the `pub(crate) use action::uninstall_to` re-export since the tests in `action.rs` that called `crate::commands::package::uninstall_to` were updated to use `super::uninstall_to` instead (the function now lives in the same module). `mod.rs` is now limited to module declarations, re-exports, and the two shared helpers (`script_extension`, `shell_command`). Added a compile-time verification test that confirms the re-exported functions have the expected signatures.

**What was changed:**

- src/commands/package/mod.rs (removed function bodies, added re-exports, added 1 test)
- src/commands/package/action.rs (added `install`, `update`, `uninstall`, `uninstall_to` functions; updated 5 test calls from `crate::commands::package::uninstall_to` to `super::uninstall_to`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- The `uninstall_to` function remains `pub(crate)` but no longer needs a re-export from `mod.rs` since all callers are now within `action.rs` or its test submodule.
- All 161 tests pass (160 existing + 1 new).

## Task: Add `depends_on` field to `PackageConfig`

**Timestamp:**

2026-04-04T10:49:00Z

**Why this task:**

First unchecked task in dependency order — the `depends_on` field is a prerequisite for all subsequent dependency-related tasks (`--depends-on` flag, `add-dep`, `remove-dep`, topological sort, etc.).

**What was done:**

Added a `depends_on: Vec<String>` field to `PackageConfig` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]` so it defaults to empty and is omitted from YAML when empty. Updated two existing test constructions that used explicit field initialization to include the new field. Added 5 new tests: parsing depends_on from YAML, default to empty, skip empty on serialize, include non-empty on serialize, and save/reload round-trip with depends_on.

**What was changed:**

- src/config.rs (added `depends_on` field, updated 2 existing test constructions, added 5 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 166 tests pass (161 existing + 5 new).
- No changes needed in plan.rs or action.rs since those use `..Default::default()` which automatically sets `depends_on` to `Vec::new()`.

## Task: Add `--depends-on` option to `homeos package add`

**Timestamp:**

2026-04-04T10:51:36Z

**Why this task:**

Next unchecked task in dependency order. The `depends_on` field was added to `PackageConfig` in the previous task; this task exposes it via the CLI.

**What was done:**

Added `--depends-on` option to the `Add` variant in `PackageCommands` using `#[arg(long = "depends-on", num_args = 1..)]` to accept one or more dependency names. Updated the `add` function signature in `registry.rs` to accept `depends_on: &[String]` and construct a `PackageConfig` with the provided dependencies. Updated all existing `add` call sites (9 in tests) to pass `&[]` for the new parameter. Added 3 new tests: add with depends_on stores dependencies, add without depends_on has empty dependencies, and add with depends_on persists after reload.

**What was changed:**

- src/main.rs (added `depends_on` field to `Add` variant, updated match arm)
- src/commands/package/registry.rs (updated `add` signature, construct `PackageConfig` with depends_on, updated 9 existing test calls, added 3 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 169 tests pass (166 existing + 3 new).
- The `num_args = 1..` attribute means `--depends-on` requires at least one value when specified, but the option itself is optional (defaults to empty Vec when omitted).

## Task: Implement `homeos package add-dep <pkg> <dep>...`

**Timestamp:**

2026-04-04T10:53:37Z

**Why this task:**

Next unchecked task in dependency order. The `depends_on` field and `--depends-on` flag are already in place; this task adds the ability to add dependencies to existing packages.

**What was done:**

Added `AddDep` variant to `PackageCommands` in `main.rs` with `package` and `deps` arguments. Implemented `add_dep` function in `registry.rs` that loads the config, validates the package exists, appends each dependency to `depends_on` (skipping duplicates with a message), and saves the config. Added 7 new tests: add single dep, add multiple deps, skip duplicate dep, error on package not found, error when not initialized, persistence after reload, and appending to existing dependencies.

**What was changed:**

- src/main.rs (added `AddDep` variant and match arm)
- src/commands/package/mod.rs (added `add_dep` to re-exports)
- src/commands/package/registry.rs (added `add_dep` function and 7 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 176 tests pass (169 existing + 7 new).
- Duplicate dependencies are skipped with an informational message rather than returning an error, consistent with how `enable` handles already-enabled packages.

## Task: Implement `homeos package remove-dep <pkg> <dep>...`

**Timestamp:**

2026-04-04T10:56:10Z

**Why this task:**

Next unchecked task in dependency order. Mirrors `add-dep` — both must exist before dependency validation can be built in later tasks.

**What was done:**

Added `RemoveDep` variant to `PackageCommands` in `main.rs` with `package` and `deps` arguments. Implemented `remove_dep` function in `registry.rs` that loads the config, validates the package exists, removes each dependency from `depends_on` (skipping with a message if not present), and saves the config. Added 7 new tests: remove single dep, remove multiple deps, skip nonexistent dep, error on package not found, error when not initialized, persistence after reload, and removing all dependencies clears the list.

**What was changed:**

- src/main.rs (added `RemoveDep` variant and match arm)
- src/commands/package/mod.rs (added `remove_dep` to re-exports)
- src/commands/package/registry.rs (added `remove_dep` function and 7 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 183 tests pass (176 existing + 7 new).
- Non-existent dependencies are skipped with an informational message rather than returning an error, consistent with how `add_dep` handles duplicate dependencies.

## Task: Enhance `homeos package remove` to reject packages depended on by others

**Timestamp:**

2026-04-04T10:58:04Z

**Why this task:**

Next unchecked task in dependency order. Dependency validation in `remove` is a prerequisite for the topological sort and dependency ordering tasks that follow.

**What was done:**

Added a dependency check to the `remove` function in `registry.rs`. Before removing a package, it scans all other packages' `depends_on` fields to find dependents. If any exist, it returns an error listing them. Added 3 new tests: reject removal when one package depends on it, reject when multiple packages depend on it, and allow removal when no packages depend on it.

**What was changed:**

- src/commands/package/registry.rs (added dependent check in `remove` function and 3 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 186 tests pass (183 existing + 3 new).
- Dependents are listed in sorted order (BTreeMap iteration) for deterministic error messages.

## Task: Rename CLI argument name for add-dep and remove-dep from `dep` to `dependency`

**Timestamp:**

2026-04-04T11:31:11Z

**Why this task:**

Next unchecked task in the PRD. Simple rename with no dependencies on other unchecked tasks.

**What was done:**

Renamed the `deps` field to `dependency` in both `AddDep` and `RemoveDep` variants of `PackageCommands` in `main.rs`. Updated the corresponding match arms to use the new field name. Added 2 unit tests verifying that the clap-generated help for `add-dep` and `remove-dep` shows `dependency` as the positional argument name.

**What was changed:**

- src/main.rs (renamed `deps` to `dependency` in struct fields and match arms, added 2 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 188 tests pass (186 existing + 2 new).
- The internal function signatures in `registry.rs` (`add_dep`, `remove_dep`) still accept `&[String]` — only the CLI-facing argument name changed, which is the intent of the task.

## Task: Implement topological sort for package install order based on `depends_on`

**Timestamp:**

2026-04-04T11:40:44Z

**Why this task:**

Next unchecked task in the PRD. The topological sort is a prerequisite for integrating dependency ordering into `homeos package install`.

**What was done:**

Created a new `topo` module (`src/topo.rs`) with a `topological_sort` function that uses Kahn's algorithm to sort packages in dependency order. The function takes a `Config` and a list of package names, builds an in-degree map considering only dependencies among the requested packages, and returns them in topological order (dependencies first). If a circular dependency is detected (not all packages can be dequeued), it returns an error listing the cycle participants. Output is deterministic — packages at the same topological level are sorted alphabetically. Added 10 unit tests covering: no dependencies, single dependency, chain, diamond, circular (2-way and 3-way), dependency outside set, empty list, single package, package not in config, and multiple dependencies.

**What was changed:**

- src/topo.rs (new — topological sort function with Kahn's algorithm and 10 tests)
- src/main.rs (added `mod topo` declaration)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 199 tests pass (188 existing + 11 new, including the module-level tests).
- The function is marked `#[allow(dead_code)]` since it will be integrated in the next task.
- Only considers dependencies among the given package set — external dependencies are ignored. This is intentional; the next task (integration) will handle expanding the set to include transitive dependencies.

## Task: Integrate dependency ordering into `homeos package install`

**Timestamp:**

2026-04-04T11:44:44Z

**Why this task:**

Next unchecked task in the PRD. Depends on the topological sort implementation completed in the previous task.

**What was done:**

Added an `expand_dependencies` function to `action.rs` that traverses `depends_on` fields transitively to collect all dependency packages. Integrated it into `run_action`: when the action is `Install`, the requested package list is expanded to include transitive dependencies, then sorted topologically via `topological_sort` before being passed to `Plan::build`. This ensures dependencies are installed before the packages that depend on them. Update and uninstall actions are unaffected — they use the original package list as-is. Removed `#[allow(dead_code)]` from `topological_sort` since it is now used.

**What was changed:**

- src/commands/package/action.rs (added `expand_dependencies` function, integrated topo sort into `run_action` for Install, added 9 tests)
- src/topo.rs (removed `#[allow(dead_code)]`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 208 tests pass (199 existing + 9 new).
- `expand_dependencies` uses a stack-based DFS to collect transitive dependencies, with a visited set to avoid duplicates.
- Dependencies not found in `config.packages` are still included in the expanded set — `Plan::build` will error on them with "Package not found".
- Circular dependencies are caught by `topological_sort` and surfaced as errors before any scripts execute.
- Only `Install` expands dependencies; `Update` and `Uninstall` operate on exactly the packages specified by the user.

## Task: Implement `homeos apply`

**Timestamp:**

2026-04-04T12:31:10Z

**Why this task:**

Next unchecked task in the PRD. No remaining dependencies — all prerequisite tasks (install, update, state management, dependency ordering) are complete.

**What was done:**

Added `homeos apply` as a top-level CLI command. The implementation loads `homeos.yml` and `state.yml`, classifies enabled packages into two groups: install (enabled + not in state) and update (enabled + in state). Disabled packages are silently skipped. Install packages go through dependency expansion and topological sort before execution. A combined plan showing both install and update targets is displayed with a single confirmation prompt. Scripts execute in order: installs first, then updates. State is updated per package after successful execution. Added `apply` and `apply_to` functions in `action.rs` (the latter with injectable I/O for testability), re-exported through `mod.rs`.

**What was changed:**

- src/main.rs (added `Apply` variant to `Commands` enum and dispatch)
- src/commands/package/action.rs (added `apply`, `apply_to`, `write_script` test helper, and 9 tests)
- src/commands/package/mod.rs (added `apply` to re-exports)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 217 tests pass (208 existing + 9 new).
- `apply` is a top-level command (not under `package`) matching the README command structure.
- The combined plan shows install and update sections separately so the user can see exactly what will happen.
- When there are no enabled packages to process, a "Nothing to do." message is shown without prompting.
- Dependency expansion only applies to the install portion; updates operate on the exact set of enabled+in-state packages.

## Task: Integrate dependency ordering into `homeos apply`

**Timestamp:**

2026-04-04T12:34:49Z

**Why this task:**

Next unchecked task in the PRD. Directly follows the `homeos apply` implementation — the apply command already existed but executed all installs before all updates without respecting dependency order across action types.

**What was done:**

Refactored `apply_to` to use a unified dependency-ordered execution flow instead of separate install-then-update phases. The new approach: (1) expands install dependencies transitively, (2) merges all packages (install + update + expanded deps) into a single set, (3) topologically sorts the unified set, (4) classifies each package as install or update based on state, (5) executes in that single dependency-respecting order. This ensures that if an install target depends on an already-installed package (being updated), the update runs first. Plans are still displayed separately (install vs update) for clear output. Added 5 tests covering: update-before-install dependency, install chain ordering, update-only ordering, transitive dependency expansion, and mixed install/update diamond dependency.

**What was changed:**

- src/commands/package/action.rs (refactored `apply_to` for unified topo-sorted execution, added 5 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 222 tests pass (217 existing + 5 new).
- The key behavioral change: previously `apply` ran all installs then all updates. Now execution interleaves install and update actions in topological order, so a dependency that needs updating runs before a dependent that needs installing.
- The display plan still shows install and update sections separately for readability — only execution order changed.
- Update packages are now also topologically sorted (previously they were not), ensuring correct order even when all packages are updates.

## Task: Add `--repo` option to CLI

**Timestamp:**

2026-04-04T12:39:04Z

**Why this task:**

Next unchecked task in the PRD. This is a prerequisite for the repo management commands (repo list, repo add, repo remove) that follow.

**What was done:**

Added a `--repo` / `-r` global CLI option (defaults to `"default"`) that selects which repository to operate on. Propagated the repo name into `Context` as a new field, replacing the hardcoded `"default"` in `default_repo_dir()`. Renamed `default_repo_dir()` to `repo_dir()` since it now resolves dynamically based on the selected repo. Updated all call sites across `init.rs`, `cd.rs`, `action.rs`, and `registry.rs`. Added 5 new tests: 3 for CLI option parsing (default value, `--repo`, `-r`) and 2 for context path resolution with custom repo names.

**What was changed:**

- src/main.rs (added `--repo` / `-r` global option, pass repo to Context, 3 new CLI tests)
- src/context.rs (added `repo` field, renamed `default_repo_dir()` to `repo_dir()`, updated constructor signature, 2 new tests)
- src/commands/init.rs (updated to use `repo_dir()`, updated fixture)
- src/commands/cd.rs (updated to use `repo_dir()`, updated fixture and error message)
- src/commands/package/action.rs (updated fixture)
- src/commands/package/registry.rs (updated fixture)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 227 tests pass (222 existing + 5 new).
- The `cd.rs` error message was updated from "Default repository not found" to "Repository not found" since it may now refer to a non-default repo.
- All existing test fixtures pass `"default"` as the repo name, preserving existing behavior.

## Task: Implement `homeos repo list`

**Timestamp:**

2026-04-04T12:42:08Z

**Why this task:**

First unchecked task remaining. No dependencies on the other two repo tasks (`repo add`, `repo remove`). This is the simplest of the three and establishes the `commands/repo` module structure.

**What was done:**

Implemented `homeos repo list` which lists all registered repositories by reading directory entries under `<base_dir>/repos/`. Output is sorted alphabetically, one repository name per line. Only directories are listed (files are ignored). If the repos directory doesn't exist, nothing is printed. Added a `Repo` subcommand with `RepoCommands::List` variant to the CLI. Created `commands/repo.rs` with a testable `list_to` pattern (writer injection) matching the existing package list pattern.

**What was changed:**

- src/commands/repo.rs (new — repo list implementation with 5 unit tests)
- src/commands.rs (added `pub mod repo`)
- src/main.rs (added `Repo` variant to `Commands` enum, `RepoCommands` enum, match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 232 tests pass (227 existing + 5 new).
- Tests cover: no repos dir, empty repos dir, single repo, multiple repos sorted, files ignored.
- The implementation follows the same `list`/`list_to` pattern used by `package list` for testability.

## Task: Implement `homeos repo add <name> <url>`

**Timestamp:**

2026-04-04T12:44:36Z

**Why this task:**

Next unchecked task in dependency order. `repo list` is complete; `repo add` is a prerequisite for `repo remove` (need repos to remove).

**What was done:**

Implemented `homeos repo add <name> <url>` which clones a remote git repository into `repos/<name>/`. The command validates that the target directory doesn't already exist, creates the `repos/` directory if needed, then runs `git clone`. Added `Add` variant to `RepoCommands` with `name` and `url` arguments, and wired it into the main match arm. Added 4 unit tests using local git repos as clone sources.

**What was changed:**

- src/commands/repo.rs (added `add` function and 4 tests; added `create_local_git_repo` test helper)
- src/main.rs (added `Add` variant to `RepoCommands`, added match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 236 tests pass (232 existing + 4 new).
- Tests cover: successful clone, repos dir auto-creation, already-exists error, invalid URL error.
- Uses `std::process::Command` to run `git clone` — no additional dependencies needed.

## Task: Implement `homeos repo remove <name>`

**Timestamp:**

2026-04-04T12:46:48Z

**Why this task:**

Only remaining unchecked task in the PRD. All dependencies (repo infrastructure, add/list) are complete.

**What was done:**

Implemented `homeos repo remove <name>` which removes the local repository directory at `repos/<name>/`. The command validates that the target directory exists, then removes it recursively with `std::fs::remove_dir_all`. Added `Remove` variant to `RepoCommands` with a `name` argument and wired it into the main match arm. Added 3 unit tests.

**What was changed:**

- src/commands/repo.rs (added `remove` function and 3 tests)
- src/main.rs (added `Remove` variant to `RepoCommands`, added match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 239 tests pass (236 existing + 3 new).
- Tests cover: successful removal, nonexistent repo error, removal doesn't affect other repos.

## Task: Enhance `homeos apply` to show disabled packages in plan

**Timestamp:**

2026-04-04T14:37:57Z

**Why this task:**

Only remaining unchecked task in the PRD (both Tasks and Post Tasks sections).

**What was done:**

Modified `apply_to` to collect disabled packages during the classification loop and display `Skipping <pkg> (disabled)` messages in the plan output. Two cases are handled:
1. When there are enabled packages: disabled messages appear after the install/update plan lines but before the confirmation prompt.
2. When all packages are disabled (nothing to do): disabled messages appear before the "Nothing to do." message.

Updated two existing tests (`test_apply_skips_disabled_packages` and `test_apply_nothing_to_do_when_all_disabled`) to expect the new disabled messages. Added three new tests:
- `test_apply_shows_disabled_in_plan_with_enabled_packages` — mixed install/update/disabled scenario
- `test_apply_shows_multiple_disabled_packages` — multiple disabled packages shown
- `test_apply_disabled_shown_before_prompt` — disabled message ordering relative to confirmation prompt

**What was changed:**

- src/commands/package/action.rs (added disabled collection in apply_to, display disabled in plan, updated 2 tests, added 3 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 242 tests pass (239 existing + 3 new).
- The disabled messages are displayed at the `apply_to` level rather than injected into `Plan::build`, since `apply` handles disabled filtering before plan construction. This keeps `Plan::build` unchanged and avoids passing disabled packages through the plan machinery.

## Task: Enhance `homeos repo remove` to guard against installed packages

**Timestamp:**

2026-04-04T16:07:46Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Enhanced `homeos repo remove` to check the target repository's `state.yml` before deletion. If the file exists and contains installed packages, the command now returns an error: "Repository '<name>' has installed packages. Uninstall them first." If `state.yml` is absent or has an empty installed list, removal proceeds as before. Added 3 tests covering: rejection when installed packages exist, allowing removal with empty state, and allowing removal without a state file.

**What was changed:**

- src/commands/repo.rs (added `State` import, added state.yml check in `remove`, added 3 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 245 tests pass (242 existing + 3 new).
- The state.yml path is constructed directly from the target repo directory (`repos_dir().join(name).join("state.yml")`) rather than using `ctx.state_path()`, since the context is configured for the current repo, not the repo being removed.

## Task: Implement `homeos package cd [<package>]`

**Timestamp:**

2026-04-04T16:10:11Z

**Why this task:**

First unchecked task in dependency order. No dependencies on other unchecked tasks.

**What was done:**

Implemented `homeos package cd [<package>]` which launches a shell in the package root directory (without argument) or a specific package directory (with argument). Added `cd` and `resolve_cd_target` functions in `registry.rs`, following the same `resolve_target` pattern as `commands/cd.rs`. The `resolve_cd_target` function validates config exists, checks the package is defined (when specified), and verifies the target directory exists. Added `Cd` variant to `PackageCommands` with an optional `package` argument and wired it in `main.rs`. Added 5 tests covering: packages root resolution, specific package resolution, unknown package error, missing directory error, and not-initialized error.

**What was changed:**

- src/commands/package/registry.rs (added `cd`, `resolve_cd_target` functions and 5 tests)
- src/commands/package/mod.rs (added `cd` to re-exports)
- src/main.rs (added `Cd` variant to `PackageCommands`, added match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 250 tests pass (245 existing + 5 new).
- The `package` argument is `Option<String>` in the CLI definition, making it optional per the README spec `cd [<package>]`.
- Reused the same shell-launch pattern from `commands/cd.rs` (`$SHELL` with fallback to `/bin/sh`).

## Task: Enhance `homeos init` to accept an optional `<url>` argument

**Timestamp:**

2026-04-04T16:13:29Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks. Prerequisite for the `--strip-git` flag task that follows.

**What was done:**

Enhanced `homeos init` to accept an optional `<url>` argument. When provided, the command clones the remote repository into the default repo directory using `git clone` instead of scaffolding an empty structure. The existing idempotency check (early return when `homeos.yml` exists) applies to both modes — if already initialized, the clone is skipped. Changed the `Init` variant in `Commands` from a unit variant to a struct variant with an optional `url` field. Updated `init::run` signature to accept `url: Option<&str>`. Updated all callers (`main.rs` dispatch, `cd.rs` test fixtures) to pass the new parameter. Added 4 new tests: clone from local git repo, idempotency with URL, invalid URL error, and repos dir auto-creation.

**What was changed:**

- src/main.rs (changed `Init` to struct variant with `url`, updated dispatch)
- src/commands/init.rs (added clone branch to `run`, added `create_local_git_repo` test helper, added 4 tests)
- src/commands/cd.rs (updated 2 test calls to pass `None`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 254 tests pass (250 existing + 4 new).
- The clone mode reuses the same `git clone` pattern from `commands/repo.rs::add`.
- Clone mode does not create `packages/`, `homeos.yml`, or `.gitignore` — those are expected to already exist in the cloned repository.
- The success message differentiates between the two modes: scaffold says "Initialized homeos at ..." while clone says "Initialized homeos at ... (cloned from ...)".

## Task: Implement `--strip-git` flag for `homeos init`

**Timestamp:**

2026-04-04T16:16:22Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks. Prerequisite for nothing but follows naturally from the `homeos init <url>` enhancement.

**What was done:**

Added a `--strip-git` flag to `homeos init` that removes the `.git` directory after cloning a remote repository. The flag is a boolean CLI argument on the `Init` variant. When `strip_git` is true and a URL is provided, the `.git` directory is removed after a successful clone. When used without a URL (scaffold mode), the flag is silently ignored. Updated the `run` function signature to accept the new `strip_git: bool` parameter and updated all existing call sites (init tests, cd tests, main dispatch). Also extracted a `create_source_repo_with_config` test helper to reduce duplication in clone-related tests.

**What was changed:**

- src/main.rs (added `strip_git` field to `Init` variant, updated dispatch)
- src/commands/init.rs (added `strip_git` parameter to `run`, added `.git` removal logic, updated all existing test calls, added `create_source_repo_with_config` helper, added 3 new tests)
- src/commands/cd.rs (updated 2 test calls to pass new parameter)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 257 tests pass (254 existing + 3 new).
- The 3 new tests cover: strip_git removes `.git` directory, strip_git false preserves it, and strip_git without URL is a no-op in scaffold mode.
- The flag has no `requires` constraint in clap — it's valid but meaningless without a URL, keeping the CLI simple.

## Task: Add `plugins` section to Config

**Timestamp:**

2026-04-04T16:20:13Z

**Why this task:**

First unchecked task in dependency order — the `plugins` section in Config is a prerequisite for all subsequent plugin-related tasks (plugin list, add, remove, and package add integration).

**What was done:**

Added a `PluginConfig` struct with a `url: String` field. Added a `plugins: BTreeMap<String, PluginConfig>` field to `Config` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]` so it defaults to empty and is omitted from YAML when empty. Updated all existing `Config` struct literals across `config.rs`, `plan.rs`, `topo.rs`, and `action.rs` that used explicit field initialization to include `..Default::default()` for the new field. Added 5 new tests: parsing plugins from YAML, default to empty, skip empty on serialize, include non-empty on serialize, and save/reload round-trip with plugins.

**What was changed:**

- src/config.rs (added `PluginConfig` struct, added `plugins` field to `Config`, updated 2 existing test constructions, added 5 new tests)
- src/plan.rs (updated 1 fixture `Config` construction)
- src/topo.rs (updated 1 fixture `Config` construction)
- src/commands/package/action.rs (updated 4 test `Config` constructions)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 262 tests pass (257 existing + 5 new).
- The `PluginConfig` struct uses `Default` derive with an empty string for `url`, consistent with how other config structs use defaults.
- Existing code that uses `Config::default()` or `..Default::default()` required no changes — only explicit struct literals needed updating.

## Task: Add `plugin` and `params` fields to PackageConfig

**Timestamp:**

2026-04-04T16:22:20Z

**Why this task:**

First unchecked task in dependency order — `plugin` and `params` fields on `PackageConfig` are prerequisites for the `--plugin` and `--params` CLI options and the plugin integration into `homeos package add`.

**What was done:**

Added two new fields to `PackageConfig`: `plugin: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]` and `params: BTreeMap<String, String>` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`. Both default to None/empty and are omitted from YAML when at their defaults. Updated 2 existing test struct literals that used explicit field initialization (without `..Default::default()`) to include the new fields. Added 5 new tests: parse plugin and params from YAML, defaults to None/empty, skip on serialize when empty, include on serialize when present, and save/reload round-trip.

**What was changed:**

- src/config.rs (added `plugin` and `params` fields to `PackageConfig`, updated 2 existing test struct literals, added 5 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 267 tests pass (262 existing + 5 new).
- Only 2 explicit `PackageConfig` struct literals needed updating — all others already used `..Default::default()` which handles the new fields automatically.
- The `params` field uses `BTreeMap<String, String>` (not `Option<BTreeMap>`) for consistency with `actions_overrides`, which uses the same pattern.

## Task: Implement `homeos plugin list`

**Timestamp:**

2026-04-04T16:25:04Z

**Why this task:**

First unchecked task in dependency order. The `plugins` section in Config and `PluginConfig` struct are already in place from previous tasks. This is a prerequisite for the remaining plugin commands (list-remote, add, remove).

**What was done:**

Implemented `homeos plugin list` which lists registered plugins from the current repository's `homeos.yml`. Created `commands/plugin.rs` with `list` and `list_to` functions following the same writer-injection pattern used by `repo list` and `package list`. Output is a formatted table with Name and URL columns. Name column width dynamically adjusts to the longest plugin name (minimum 4 for "Name" header). Plugins are listed in alphabetical order (BTreeMap iteration). Empty plugins section produces no output. Added `Plugin` subcommand with `PluginCommands::List` variant to the CLI and wired the dispatch in `main.rs`.

**What was changed:**

- src/commands/plugin.rs (new — plugin list implementation with 6 unit tests)
- src/commands.rs (added `pub mod plugin`)
- src/main.rs (added `Plugin` variant to `Commands`, `PluginCommands` enum, match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 273 tests pass (267 existing + 6 new).
- Tests cover: no plugins, single plugin, multiple plugins sorted, table header format, name column width adjustment, and error when not initialized.
- The `PluginCommands` enum currently has only `List` — subsequent tasks will add `ListRemote`, `Add`, and `Remove` variants.

## Task: Implement `homeos plugin list-remote`

**Timestamp:**

2026-04-04T16:29:15Z

**Why this task:**

First unchecked task in dependency order. The `plugins` section in Config and `plugin list` are already in place. This is a prerequisite for `plugin add` which needs to resolve plugin URLs from GitHub.

**What was done:**

Implemented `homeos plugin list-remote` which fetches `hainet50b/homeos-plugin-*` repositories from the GitHub Search API and displays them in a table with Name, Description, and URL columns. Added `reqwest` (with `blocking` and `json` features) and `serde_json` as dependencies. The implementation uses a testable architecture: `list_remote_to` accepts a generic fetch function, allowing tests to inject mock data without network calls. The real `fetch_remote_plugins` function calls the GitHub API with a `User-Agent` header, filters results to only `homeos-plugin-` prefixed repos, and strips the prefix for display names. Network errors propagate as `Err` and are displayed to the user. Added `ListRemote` variant to `PluginCommands` and wired the dispatch in `main.rs`.

**What was changed:**

- Cargo.toml (added `reqwest` with blocking+json features, added `serde_json`)
- src/commands/plugin.rs (added `GitHubSearchResponse`, `GitHubRepo`, `RemotePlugin` structs, `fetch_remote_plugins`, `list_remote`, `list_remote_to` functions, 7 new tests)
- src/main.rs (added `ListRemote` variant to `PluginCommands`, added match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 280 tests pass (273 existing + 7 new).
- Tests cover: no plugins found, single plugin, multiple plugins, table header format, name column width adjustment, empty description handling, and fetch error propagation.
- The fetch function is injected as a generic closure parameter to `list_remote_to`, keeping tests fast and deterministic without network access.
- GitHub API results are filtered client-side with `starts_with("homeos-plugin-")` to ensure only matching repos are displayed, since the search API may return partial matches.

## Task: Implement `homeos plugin add <name> [<url>]`

**Timestamp:**

2026-04-04T16:32:17Z

**Why this task:**

First unchecked task in dependency order. `plugin list` and `plugin list-remote` are already in place. This is a prerequisite for `plugin remove` and the plugin integration into `package add`.

**What was done:**

Implemented `homeos plugin add <name> [<url>]` which registers a plugin in `homeos.yml` and clones it into `plugins/<name>/`. Without a URL, the default is resolved as `https://github.com/hainet50b/homeos-plugin-<name>`. Added `plugins_dir()` method to `Context` for resolving the plugins directory path. The implementation checks for duplicate plugin names in config and existing plugin directories before cloning. On successful clone, the plugin entry is saved to `homeos.yml`. Added `Add` variant to `PluginCommands` with `name` (required) and `url` (optional) arguments.

**What was changed:**

- src/context.rs (added `plugins_dir()` method and test)
- src/commands/plugin.rs (added `add` function with 8 unit tests, added `fixture_with_config` and `create_local_git_repo` test helpers)
- src/main.rs (added `Add` variant to `PluginCommands`, added match arm for dispatch)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 289 tests pass (280 existing + 9 new).
- Tests cover: successful clone and registration, default URL resolution (verified via git clone error), duplicate plugin name rejection, directory already exists rejection, invalid URL error, plugins directory auto-creation, and error when not initialized.
- The `test_add_resolves_default_url` test verifies default URL behavior by checking that a clone attempt is made (and fails) when no URL is provided, confirming the URL construction logic works without requiring network access.

## Task: Implement `homeos plugin remove <name>`

**Timestamp:**

2026-04-04T16:34:35Z

**Why this task:**

First unchecked task in the PRD. The `plugin add` and `plugin list` commands are already in place; `plugin remove` is the natural counterpart.

**What was done:**

Implemented `homeos plugin remove <name>` which removes the plugin directory at `plugins/<name>/` and its entry from `homeos.yml`. Before removal, the function scans all packages' `plugin` fields to find references to the plugin being removed. If any packages reference it, a warning is printed to stderr listing them, but removal proceeds (warn, not block). If the plugin directory doesn't exist (e.g., manually deleted), removal still succeeds by cleaning up the config entry. Added `Remove` variant to `PluginCommands` with a `name` argument and wired the dispatch in `main.rs`.

**What was changed:**

- src/commands/plugin.rs (added `remove` function and 6 tests)
- src/main.rs (added `Remove` variant to `PluginCommands`, added match arm)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 295 tests pass (289 existing + 6 new).
- Tests cover: successful removal (directory + config), plugin not found error, removal without directory, warning when packages reference plugin (still succeeds), does not affect other plugins, and error when not initialized.
- Referencing packages are listed in sorted order (BTreeMap iteration) for deterministic warning messages.

## Task: Add `--plugin` and `--params` options to `homeos package add` CLI definition

**Timestamp:**

2026-04-04T16:37:58Z

**Why this task:**

First unchecked task in the PRD. This is the CLI definition prerequisite for the next task which integrates plugin templates into `homeos package add`.

**What was done:**

Added `--plugin <name>` and `--params <key=value>...` options to the `Add` variant of `PackageCommands` in `main.rs`. The `--plugin` option is an `Option<String>` and `--params` accepts one or more `key=value` pairs parsed by a custom `parse_key_value` function that splits on the first `=`. Updated the `add` function in `registry.rs` to accept `plugin: Option<&str>` and `params: &BTreeMap<String, String>`, storing them in the `PackageConfig`. Updated all 12 existing `add` call sites in tests to pass the new parameters. Added a `BTreeMap` import to the test module in `registry.rs`. Added 3 CLI parsing tests in `main.rs` and 4 registry tests in `registry.rs`.

**What was changed:**

- src/main.rs (added `parse_key_value` function, added `plugin` and `params` fields to `Add` variant, updated dispatch with `BTreeMap` conversion, added 3 CLI tests)
- src/commands/package/registry.rs (updated `add` signature with `plugin` and `params` parameters, store in `PackageConfig`, updated 12 test calls, added `BTreeMap` import to test module, added 4 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 302 tests pass (295 existing + 3 new CLI tests + 4 new registry tests).
- The `parse_key_value` function splits on the first `=` only, allowing values to contain `=` characters.
- The `--params` option uses `Vec<(String, String)>` at the CLI level, converted to `BTreeMap` in the dispatch for deterministic ordering in config.
- This task only adds the CLI definition and storage — the actual template integration (loading templates, replacing placeholders) is the next task.

## Task: Integrate plugin into homeos package add

**Timestamp:**

2026-04-04T16:42:23Z

**Why this task:**

This is the last remaining unchecked task in the PRD. All previous tasks are complete.

**What was done:**

- Added `PluginManifest` struct to `config.rs` for parsing `params.yml` files from plugin directories
- Modified `add` in `registry.rs` to branch on `--plugin`: when specified, loads the plugin directory, validates required params against `params.yml`, reads OS-appropriate templates (`<action>.sh.tmpl` or `<action>.ps1.tmpl`), replaces `{{key}}` placeholders with param values, and writes rendered scripts. When no plugin is specified, generates skeleton scripts as before.
- Extracted `generate_skeleton_scripts` and `generate_plugin_scripts` helper functions
- Added `render_template` function for `{{key}}` placeholder replacement
- Updated 3 existing tests to set up plugin directories (previously they only stored plugin/params in config without needing the plugin dir to exist)
- Added 8 new plugin integration tests covering: template rendering, missing templates skipped, plugin not found error, missing required params error, multiple params replacement, preserving existing scripts, no params.yml skips validation, no templates creates no scripts
- Added 4 new `PluginManifest` tests in `config.rs`

**What was changed:**

- src/config.rs (added PluginManifest struct, load method, and 4 tests)
- src/commands/package/registry.rs (refactored add function, added generate_skeleton_scripts, generate_plugin_scripts, render_template, updated 3 existing tests, added 8 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 326 tests pass (314 existing + 12 new tests).
- The plugin directory must exist before `package add --plugin` is called; the error message guides the user to run `homeos plugin add` first.
- `params.yml` is optional in the plugin directory — if absent, no parameter validation is performed.
- Templates that don't exist for the current OS are silently skipped, matching the PRD requirement.
- Existing scripts in the package directory are preserved even when using plugin templates, consistent with the existing skeleton script behavior.

---

# Ralph Loop — 2026-04-05T04:03:41Z

## Task: Fix shell detection for Windows support

**Timestamp:**

2026-04-05T04:06:07Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Extracted a shared `detect_shell()` function into `commands.rs` that uses the `SHELL` env var if set, otherwise falls back to `pwsh` on Windows or `/bin/sh` on Unix. Replaced the duplicated inline shell detection in both `cd.rs` and `package/registry.rs` with calls to the shared helper. Added a testable `resolve_shell()` inner function to avoid unsafe env var manipulation in tests. Added 2 unit tests. Also fixed a pre-existing test bug in `package/mod.rs` where `test_shell_command_returns_os_appropriate_value` expected `"powershell"` but `shell_command()` returns `"pwsh"`.

**What was changed:**

- src/commands.rs (added `detect_shell`, `resolve_shell`, and 2 tests)
- src/commands/cd.rs (replaced inline shell detection with `detect_shell()`)
- src/commands/package/registry.rs (replaced inline shell detection with `detect_shell()`)
- src/commands/package/mod.rs (fixed test expectation from `"powershell"` to `"pwsh"`)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 316 tests pass.
- Used a `resolve_shell(shell_env: Option<String>)` inner function to make the logic testable without `unsafe` env var manipulation (Rust 2024 edition marks `set_var`/`remove_var` as unsafe).
- The helper is placed in `commands.rs` rather than `package/mod.rs` since it's used by both `commands::cd` and `commands::package::registry` — keeping it at the `commands` level avoids an awkward cross-module dependency.

## Task: Validate cloned repository in `homeos init <url>`

**Timestamp:**

2026-04-05T04:09:04Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Added validation after `git clone` in `homeos init <url>` to check that `homeos.yml` exists in the cloned directory. If it does not exist, the cloned directory is removed and an error "Not a valid homeos repository" is returned. Also fixed an existing test (`test_init_with_url_creates_repos_dir`) that was cloning a repo without `homeos.yml` — updated it to use the `create_source_repo_with_config` helper. Added 2 new tests: one verifying the error message and one verifying cleanup of the cloned directory.

**What was changed:**

- src/commands/init.rs (added validation logic after clone, fixed existing test, added 2 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 318 tests pass (316 existing + 2 new).
- The validation is placed between the clone success check and the `--strip-git` logic, so invalid repos are rejected before any further processing.
- The existing test `test_init_with_url_creates_repos_dir` was using `create_local_git_repo` (no `homeos.yml`) — now correctly uses `create_source_repo_with_config` to include a valid `homeos.yml`.

## Task: Validate cloned plugin in `homeos plugin add`

**Timestamp:**

2026-04-05T04:12:50Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Added validation after `git clone` in `homeos plugin add` to check that `params.yml` exists in the cloned directory. If it does not exist, the cloned directory is removed and an error "Not a valid homeos plugin" is returned. Added a `create_local_plugin_repo` test helper that creates a git repo with `params.yml` committed. Updated 3 existing tests (`test_add_clones_and_registers_plugin`, `test_add_default_url_without_explicit_url`, `test_add_creates_plugins_dir`) to use the new helper since they now need `params.yml` to pass validation. Added 2 new tests: one verifying the error message and one verifying cleanup of the cloned directory and that config is not modified.

**What was changed:**

- src/commands/plugin.rs (added params.yml validation after clone, added `create_local_plugin_repo` helper, updated 3 existing tests, added 2 new tests)
- src/commands/package/action.rs (cargo fmt formatting only)
- src/commands/repo.rs (cargo fmt formatting only)
- src/context.rs (cargo fmt formatting only)
- src/main.rs (cargo fmt formatting only)
- src/topo.rs (cargo fmt formatting only)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 320 tests pass (318 existing + 2 new).
- The validation is placed between the clone success check and the config update, mirroring the same pattern used in `homeos init <url>` for `homeos.yml` validation.
- The previous session had implemented the code changes but not completed the commit workflow. This session verified, formatted, and committed the work.

## Task: Add URL existence check before `git clone` in `homeos plugin add`

**Timestamp:**

2026-04-05T04:15:07Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Added a `check_repo_exists` function that calls the GitHub API (`GET /repos/hainet50b/homeos-plugin-<name>`) to verify the repository exists before attempting `git clone`. This check only runs when the URL is auto-resolved (no explicit URL provided). If the API returns 404, the function returns an error with a clear message: "Plugin '<name>' not found on GitHub (homeos-plugin-<name>)".

Refactored `add` into `add` (public, calls `check_repo_exists`) and `add_with` (internal, accepts an injectable `repo_checker` function) to make the GitHub API check testable without network access.

Updated the existing `test_add_resolves_default_url` test to use `add_with` with an injected checker. Added 3 new tests:
- `test_add_auto_resolved_url_checks_repo_exists` — verifies error message from checker
- `test_add_auto_resolved_url_skips_check_with_explicit_url` — verifies checker is NOT called when URL is explicit (panics if called)
- `test_add_auto_resolved_url_no_clone_on_check_failure` — verifies no clone directory is created when checker fails

**What was changed:**

- src/commands/plugin.rs (added `check_repo_exists`, refactored `add` into `add`/`add_with`, updated 1 test, added 3 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 323 tests pass (320 existing + 3 new).
- The `check_repo_exists` function is placed immediately before `add` since it's a helper specific to that function, consistent with the pattern used elsewhere (e.g., `fetch_remote_plugins` before `list_remote`).
- The `auto_resolved` flag tracks whether the URL was provided or auto-generated, so the check only fires for the convention-based default URL path.

## Task: Implement `--local` flag for `homeos plugin add`

**Timestamp:**

2026-04-05T04:18:53Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Added `--local` flag to `homeos plugin add` CLI definition. When `--local` is specified, instead of cloning a remote repository, it creates an empty plugin skeleton in `plugins/<name>/` containing:
- `params.yml` with empty params list
- OS-appropriate template files (`install.sh.tmpl`, `update.sh.tmpl`, `uninstall.sh.tmpl` on Linux/macOS; `.ps1.tmpl` on Windows)

Each template contains a comment noting it was generated by homeos. The plugin is registered in `homeos.yml` with an empty URL. When `--local` is used, URL and repo checker are ignored.

Added `add_local` function in `plugin.rs` that handles the skeleton creation. Updated `add` and `add_with` signatures to accept the `local` boolean parameter.

**What was changed:**

- src/main.rs (added `--local` flag to `PluginCommands::Add`, wired `local` param, added 2 CLI tests)
- src/commands/plugin.rs (added `add_local` function, updated `add`/`add_with` signatures, updated all existing test calls, added 7 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 332 tests pass (323 existing + 9 new).
- The `add_local` function reuses `script_extension()` from `commands::package::mod` for OS-appropriate template file extensions.
- When `--local` is used with a URL argument, `--local` takes precedence and the URL is ignored (tested).
- The empty URL in `homeos.yml` distinguishes local plugins from cloned ones.

## Task: Remove `.git` directory from cloned plugins after `homeos plugin add`

**Timestamp:**

2026-04-05T04:21:18Z

**Why this task:**

Only remaining unchecked task in the PRD. Completes all Tasks.

**What was done:**

Added `.git` directory removal in `add_with()` after successful clone and validation. After verifying `params.yml` exists, the code now checks for a `.git` directory in the cloned plugin and removes it with `fs::remove_dir_all`. This matches the same approach used by `init --strip-git`. Added a unit test `test_add_removes_git_directory_after_clone` that clones a local plugin repo and asserts the `.git` directory is absent afterward.

**What was changed:**

- src/commands/plugin.rs (added `.git` removal after clone validation, added 1 new test)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 333 tests pass (332 existing + 1 new).
- Unlike `init --strip-git` which uses an opt-in flag, plugin add always strips `.git` since plugins are meant to be embedded in the repository, not maintained as separate git repos.

---

# Ralph Loop — 2026-04-08T07:06:34Z

## Task: Warn when executing unmodified skeleton scripts

**Timestamp:**

2026-04-08T07:13:56Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Added unmodified skeleton script detection to the `Plan` struct. Before displaying the plan, each enabled package's script is checked for the "Generated by homeos" marker. If found, a warning is shown inline in the plan display (e.g., `neovim (warning: install.sh is unmodified)`). Execution is not blocked.

Implementation details:
- Added `warnings: BTreeMap<String, Vec<String>>` field to `Plan`
- Added `Plan::check_unmodified_scripts()` method that reads each enabled package's resolved script file and checks for the marker
- Updated `Plan::display()` to show warnings inline after package names
- Added `resolve_script_name()` helper in `plan.rs` (mirrors the one in `action.rs`) to correctly resolve overridden action scripts
- Integrated `check_unmodified_scripts` calls in both `run_action` and `apply_to`

**What was changed:**

- src/plan.rs (added `warnings` field, `check_unmodified_scripts` method, `resolve_script_name` helper, updated `display`, added 6 new tests, updated all existing Plan struct literals)
- src/commands/package/action.rs (integrated `check_unmodified_scripts` in `run_action` and `apply_to`, added 4 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 342 tests pass (333 existing + 9 new).
- `PackageConfig::Default` sets `enabled` to `false` (Rust `bool::default()`), not `true` (the serde default). Tests creating `PackageConfig` with `..Default::default()` must explicitly set `enabled: true` if they need the package to be enabled.
- The `resolve_script_name` function is duplicated between `plan.rs` and `action.rs`. Both need it: `plan.rs` for warning checks, `action.rs` for script execution. A future refactor could extract it to a shared location.

---

## Task: Respect dependency order in `homeos package uninstall`

**Timestamp:**

2026-04-08T07:18:21Z

**Why this task:**

First unchecked task in the Tasks section. No dependencies on other unchecked tasks.

**What was done:**

Extended `run_action` to expand dependencies and topologically sort for `Action::Uninstall`, then reverse the order so dependents are uninstalled before their dependencies. Previously, dependency expansion only applied to `Action::Install`. Now the match on action uses three arms: `Install` (expand + topo sort), `Uninstall` (expand + topo sort + reverse), and `Update` (pass through as-is).

For example, if `neovim` depends on `git` and both are installed, `homeos package uninstall neovim` will expand to include `git`, sort to `[git, neovim]`, reverse to `[neovim, git]`, and uninstall neovim first. Dependencies not in `state.yml` are skipped via the existing plan logic.

**What was changed:**

- src/commands/package/action.rs (changed `if/else` to `match` for dependency expansion in `run_action`, added 5 new tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 347 tests pass (342 existing + 5 new).
- The 5 new tests cover: reverse order for simple dependency, chain dependency (c→b→a), skipping not-installed dependencies, circular dependency error, and state removal of expanded dependencies.

---

# Ralph Loop — 2026-04-08T07:33:44Z

## Task: Move check_unmodified_scripts into Plan::build

**Timestamp:**

2026-04-08T07:37:19Z

**Why this task:**

First unchecked task in the PRD. No dependencies on other unchecked tasks.

**What was done:**

Moved the `check_unmodified_scripts` logic into `Plan::build` by adding an optional `packages_dir: Option<&Path>` parameter. When provided, `build` checks enabled packages for unmodified skeleton scripts and populates warnings at construction time. Removed the `&mut self` method `check_unmodified_scripts`, making `Plan` immutable after construction. Updated all call sites in `action.rs` to pass `Some(&ctx.packages_dir())` and changed `let mut plan` to `let plan`. Updated all tests: warning-related tests now pass `packages_dir` directly to `build`, and non-warning tests pass `None`.

**What was changed:**

- src/plan.rs (merged `check_unmodified_scripts` into `build`, added `packages_dir` param, removed `&mut self` method, updated all tests)
- src/commands/package/action.rs (updated 3 call sites to pass `packages_dir`, removed `check_unmodified_scripts` calls, changed `mut` bindings to immutable)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 347 tests pass (no new tests added — existing tests were refactored to use the new `build` signature).
- Added explicit type annotation `BTreeMap<String, Vec<String>>` for the `warnings` variable inside `build` to satisfy type inference when `packages_dir` is `None`.
- The `resolve_script_name` function remains a private helper in `plan.rs`, used by the build logic. It is still duplicated in `action.rs` for script execution.



---

# Ralph Loop — 2026-04-08T07:42:52Z

## Task: Ensure all commands produce meaningful stdout output

**Timestamp:**

2026-04-08T07:46:46Z

**Why this task:**

First unchecked task in the Tasks section. No dependencies on other unchecked tasks.

**What was done:**

Reviewed all commands for stdout output. Most commands already had meaningful output. Three list commands were silent when their collections were empty, inconsistent with `plugin list-remote` which prints "No remote plugins found." Added empty-state messages:
- `package list` → "No packages."
- `plugin list` → "No plugins."
- `repo list` → "No repositories." (both when repos dir doesn't exist and when it exists but is empty)

Updated 3 existing tests that expected empty output to expect the new messages.

**What was changed:**

- src/commands/package/registry.rs (added "No packages." for empty list, updated test)
- src/commands/plugin.rs (added "No plugins." for empty list, updated test)
- src/commands/repo.rs (added "No repositories." for empty list in two branches, updated 2 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 347 tests pass (no new tests added — existing tests were updated to match new behavior).
- Commands that launch interactive shells (`cd`, `package cd`) intentionally produce no output, which is appropriate for their behavior.
- All other commands already had meaningful output: init prints path, add/remove/enable/disable print confirmation, install/update/uninstall show plans and progress, cat shows scripts.

---

## Task: Rename plugin config file from params.yml to plugin.yml

**Timestamp:**

2026-04-08T07:51:54Z

**Why this task:**

First unchecked task in the Tasks section. No dependencies on other unchecked tasks.

**What was done:**

Renamed all references to `params.yml` to `plugin.yml` across the codebase:
- `plugin add --local` skeleton generation now creates `plugin.yml`
- `plugin add` clone validation now checks for `plugin.yml`
- `package add` manifest loading now reads `plugin.yml`
- Updated comment in config.rs test from `params.yml` to `plugin.yml`
- Renamed test helper `create_local_plugin_repo` to write and commit `plugin.yml`
- Renamed 4 test functions that referenced `params_yml` to use `plugin_yml`
- Updated all test assertions and fixture file references

**What was changed:**

- src/config.rs (updated test path reference from `params.yml` to `plugin.yml`)
- src/commands/plugin.rs (updated `add_local` skeleton generation, `add_with` validation check, `create_local_plugin_repo` test helper, renamed 3 test functions, updated all test assertions)
- src/commands/package/registry.rs (updated manifest loading path, renamed 1 test function, updated test fixture file references)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 347 tests pass (no new tests added — existing tests were updated to reference `plugin.yml`).
- The `PluginManifest` struct name in config.rs was not renamed as it already reflects its purpose (loading plugin manifests) and the task only called for renaming the file, not the struct.

---

## Task: Implement `homeos plugin cat <name>`

**Timestamp:**

2026-04-08T07:54:43Z

**Why this task:**

First unchecked task in the Tasks section. No dependencies on other unchecked tasks.

**What was done:**

Implemented `homeos plugin cat <name>` command that displays `plugin.yml` and all template files for a specified plugin with filename headers (`=== filename ===`). Shows `(not found)` if `plugin.yml` is missing. Only displays template files that exist on disk, checking all combinations of actions (install, update, uninstall) and extensions (.sh.tmpl, .ps1.tmpl). Added `Cat` variant to `PluginCommands` enum and wired it into the main match.

**What was changed:**

- src/main.rs (added `Cat` variant to `PluginCommands`, added match arm to dispatch `commands::plugin::cat`)
- src/commands/plugin.rs (added `cat` and `cat_to` functions, added 6 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 353 tests pass (6 new tests added).
- Template files are only shown if they exist on disk, avoiding clutter for plugins that only support one OS.
- The ordering of plugin commands (list, list-remote, add, remove, cat) matches the README reference section.

---

## Task: Implement `homeos plugin cd [<name>]`

**Timestamp:**

2026-04-08T07:57:22Z

**Why this task:**

First unchecked task in the Tasks section. No dependencies on other unchecked tasks.

**What was done:**

Implemented `homeos plugin cd [<name>]` command that launches a shell in the plugins root directory (no argument) or in a specific plugin directory (with name). Added `Cd` variant to `PluginCommands` enum with optional `name` argument, wired it into the main match dispatch. Implemented `cd` and `resolve_cd_target` functions in `plugin.rs`, following the same pattern as `package cd`. The `resolve_cd_target` function validates the plugin exists in config (when name given) and that the target directory exists on disk.

**What was changed:**

- src/main.rs (added `Cd` variant to `PluginCommands`, added match arm to dispatch `commands::plugin::cd`)
- src/commands/plugin.rs (added `cd` and `resolve_cd_target` functions, added 6 tests)
- prd.md (checked off task)
- progress.md (added this entry)

**Remarks:**

- All 359 tests pass (6 new tests added).
- Function ordering in plugin.rs matches README reference order: list, list-remote, add, remove, cat, cd.
- The implementation mirrors the `package cd` pattern: separate `resolve_cd_target` for testability, `cd` for shell launch.


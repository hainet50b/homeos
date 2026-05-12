# Command Output Specification

This file defines all user-facing output for homeos commands. When adding or changing messages, follow this specification to maintain consistency.

Each table has three columns: **Condition**, **Dest** (stdout or stderr), and **Output**. Errors from main.rs are sent to stderr via `eprintln!("Error: {e}");`. Some errors during script execution are sent to stdout to maintain output flow.

## homeos init

| Condition | Dest | Output |
|-----------|------|--------|
| Scaffold success | stdout | `Initialized homeos at {path}` |
| Clone success | stdout | `Initialized homeos at {path} (cloned from {url})` |
| Already initialized (error) | stderr | `Error: Already initialized at {path}` |
| Repository directory already exists (error) | stderr | `Error: Repository directory already exists at {path}` |
| git clone fails (error) | stderr | `Error: git clone failed: {stderr}` |
| Cloned repository has no homeos.yml (error) | stderr | `Error: Not a valid homeos repository. Cloned directory removed.` |

## homeos cd

| Condition | Dest | Output |
|-----------|------|--------|
| Repositories directory not found (error) | stderr | `Error: Repositories directory not found at {path}. Run 'homeos init' first.` |

## homeos apply

| Condition | Dest | Output |
|-----------|------|--------|
| Plan display | stdout | (see Plan Display section below) |
| Dry-run (`--dry-run`) | stdout | Plan display only; exits without prompt or execution |
| User confirms | stdout | Executes with progress messages |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Installing {name}...` / `done` or `Error:` then `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Some packages fail | stdout | `Some packages failed` |

## homeos package list

| Condition | Dest | Output |
|-----------|------|--------|
| Any | stdout | Table: `Package`, `Enabled`, `Installed`, `Dependencies` columns (empty table if no packages) |

## homeos package add

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Added package '{name}'` |
| Package already in homeos.yml (error) | stderr | `Error: Package '{name}' already exists` |
| Package directory already exists (error) | stderr | `Error: Package directory '{name}' already exists. Remove it first to re-create.` |
| Plugin not found (error) | stderr | `Error: Plugin '{name}' not found. Add it first with: homeos plugin add {name}` |
| Missing plugin params (error) | stderr | `Error: Missing required plugin parameters: {params}` |
| Invalid key=value pair (error) | stderr | `Error: invalid key=value pair: no '=' found in '{input}'` |
| Dependency not found (error) | stderr | `Error: Dependency '{dependency}' not found in homeos.yml` |
| Circular dependency (error) | stderr | `Error: Circular dependency detected among packages: {names}` |

## homeos package remove

| Condition | Dest | Output |
|-----------|------|--------|
| Confirmation prompt | stdout | `The following packages will be removed from homeos.yml:` / `  {name}` |
| With --purge | stdout | `The following directories will be deleted:` / `  {path}` |
| User declines | stdout | `Aborted.` |
| Success | stdout | `Removed package '{name}'` |
| Success with --purge | stdout | `Removed package '{name}' and removed directory` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Package is installed (error) | stderr | `Error: Package '{name}' is currently installed. Uninstall it first with: homeos package uninstall {name}` |
| Package is depended on (error) | stderr | `Error: Cannot remove package '{name}' because it is depended on by: {dependents}` |

## homeos package rename

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Renamed package '{old}' to '{new}'` |
| Updated dependent (per reference) | stdout | `Updated '{dependent}' dependency: {old} → {new}` |
| Package not found (error) | stderr | `Error: Package '{old}' not found` |
| New name already exists (error) | stderr | `Error: Package '{new}' already exists` |

## homeos package add-dep

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Package '{name}' now depends on '{dependency}'` |
| Already depends | stdout | `Package '{name}' already depends on '{dependency}'` |
| Dependency not found (error) | stderr | `Error: Dependency '{dependency}' not found in homeos.yml` |
| Circular dependency (error) | stderr | `Error: Circular dependency detected among packages: {names}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package remove-dep

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Package '{name}' no longer depends on '{dependency}'` |
| Not a dependency | stdout | `Package '{name}' does not depend on '{dependency}'` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package add-alias

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Added alias '{target}={source}' to package '{name}'` |
| Already has alias | stdout | `Package '{name}' already has alias '{target}'` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package remove-alias

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Removed alias '{target}' from package '{name}'` |
| Alias not found | stdout | `Package '{name}' does not have alias '{target}'` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package enable

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Enabled package '{name}'` |
| Already enabled | stdout | `Package '{name}' is already enabled` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package disable

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Disabled package '{name}'` |
| Already disabled | stdout | `Package '{name}' is already disabled` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package info

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | Package details: enabled, installed, plugin, params, dependencies, dependents, script aliases |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package cat

| Condition | Dest | Output |
|-----------|------|--------|
| Script exists | stdout | `=== {filename} ===` / `{content}` |
| Script not found | stdout | `=== {filename} ===` / `(not found)` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package cd

| Condition | Dest | Output |
|-----------|------|--------|
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Directory not found (error) | stderr | `Error: Directory not found at {path}` |

## homeos package install

| Condition | Dest | Output |
|-----------|------|--------|
| Plan display | stdout | (see Plan Display section below) |
| Dry-run (`--dry-run`) | stdout | Plan display only; exits without prompt or execution |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Installing {name}...` / `done` or `Error:` then `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Some packages fail | stdout | `Some packages failed` |

## homeos package update

| Condition | Dest | Output |
|-----------|------|--------|
| Plan display | stdout | (see Plan Display section below) |
| Dry-run (`--dry-run`) | stdout | Plan display only; exits without prompt or execution |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Updating {name}...` / `done` or `Error:` then `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Some packages fail | stdout | `Some packages failed` |

## homeos package uninstall

| Condition | Dest | Output |
|-----------|------|--------|
| Plan display | stdout | (see Plan Display section below) |
| Dry-run (`--dry-run`) | stdout | Plan display only; exits without prompt or execution |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Uninstalling {name}...` / `done` or `Error:` then `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Some packages fail | stdout | `Some packages failed` |

## homeos plugin list

| Condition | Dest | Output |
|-----------|------|--------|
| Any | stdout | Table: `Name`, `URL` columns (empty table if no plugins). `URL` shows `(local)` when the plugin has no remote URL. |

## homeos plugin list-remote

| Condition | Dest | Output |
|-----------|------|--------|
| No remote plugins | stdout | `No remote plugins found.` |
| Has plugins | stdout | Table: `Name`, `Description`, `URL` columns |

## homeos plugin add

| Condition | Dest | Output |
|-----------|------|--------|
| Local success | stdout | `Plugin '{name}' created locally` |
| Clone success | stdout | `Plugin '{name}' added successfully` |
| Plugin not found on GitHub (error) | stderr | `Error: Plugin '{name}' not found on GitHub (homeos-plugin-{name})` |
| Plugin already in homeos.yml (error) | stderr | `Error: Plugin '{name}' already exists` |
| Plugin directory already exists (error) | stderr | `Error: Plugin directory '{name}' already exists` |
| git clone fails (error) | stderr | `Error: git clone failed: {stderr}` |
| Cloned plugin has no plugin.yml (error) | stderr | `Error: Not a valid homeos plugin. Cloned directory removed.` |

## homeos plugin remove

| Condition | Dest | Output |
|-----------|------|--------|
| Confirmation prompt | stdout | `The following plugins will be removed from homeos.yml:` / `  {name}` |
| With --purge | stdout | `The following directories will be deleted:` / `  {path}` |
| Warning about referencing packages | stdout | `Warning: the following packages reference plugin '{name}': {packages}` |
| User declines | stdout | `Aborted.` |
| Success | stdout | `Removed plugin '{name}'` |
| Success with --purge | stdout | `Removed plugin '{name}' and removed directory` |
| Plugin not found (error) | stderr | `Error: Plugin '{name}' not found` |

## homeos plugin cat

| Condition | Dest | Output |
|-----------|------|--------|
| plugin.yml exists | stdout | `=== plugin.yml ===` / `{content}` |
| plugin.yml not found | stdout | `=== plugin.yml ===` / `(not found)` |
| Template exists | stdout | `=== {filename} ===` / `{content}` |
| Template not found | stdout | `=== {filename} ===` / `(not found)` |
| Plugin not found (error) | stderr | `Error: Plugin '{name}' not found` |

## homeos plugin cd

| Condition | Dest | Output |
|-----------|------|--------|
| Plugin not found (error) | stderr | `Error: Plugin '{name}' not found` |
| Directory not found (error) | stderr | `Error: Directory not found at {path}` |

## homeos repo list

| Condition | Dest | Output |
|-----------|------|--------|
| Any | stdout | Table: repository names (empty table if no repositories) |

## homeos repo add

| Condition | Dest | Output |
|-----------|------|--------|
| Create success | stdout | `Repository '{name}' created` |
| Clone success | stdout | `Repository '{name}' cloned successfully` |
| Repository already exists (error) | stderr | `Error: Repository '{name}' already exists` |
| git clone fails (error) | stderr | `Error: git clone failed: {stderr}` |

## homeos repo cd

| Condition | Dest | Output |
|-----------|------|--------|
| Repository not found (error) | stderr | `Error: Repository '{name}' does not exist` |

## homeos repo remove

| Condition | Dest | Output |
|-----------|------|--------|
| Confirmation prompt | stdout | `Remove repository '{name}'?` |
| User declines | stdout | `Aborted.` |
| Success | stdout | `Repository '{name}' removed` |
| Removing default (error) | stderr | `Error: Cannot remove the default repository.` |
| Repository not found (error) | stderr | `Error: Repository '{name}' does not exist` |
| Has installed packages (error) | stderr | `Error: Repository '{name}' has installed packages. Uninstall them first.` |

## Plan Display

Used by `apply`, `install`, `update`, `uninstall`. Always displayed regardless of whether there are packages to execute.

When there are packages to execute:

```
The following packages will be {installed|updated|uninstalled}:
  {name}
  {name} (plugin: {plugin_name})
  {name} (warning: {script} is unmodified)
  {name} (required by {package})    # install/apply only — pulled in as a forward dependency
  {name} (depends on {package})     # uninstall only — pulled in as a reverse dependency
The following packages will be skipped:
  {name} (disabled)
  {name} (already installed)
  {name} (not installed)
  {name} (circular dependency)
  {name} (dependency disabled: {dep})    # install/apply only — dep chain includes a disabled package

Proceed? [y/N]
```

When all packages are skipped (no confirmation prompt):

```
The following packages will be skipped:
  {name} (disabled)
  {name} (already installed)
  {name} (circular dependency)
  {name} (dependency disabled: {dep})

Nothing to do.
```

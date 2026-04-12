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
| User confirms | stdout | Executes with progress messages |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Installing {name}...` / `done` or `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Some packages fail | stdout | `Some packages failed` |

## homeos package list

| Condition | Dest | Output |
|-----------|------|--------|
| No packages | stdout | `No packages.` |
| Has packages | stdout | Table: `Package`, `Enabled`, `Installed` columns |

## homeos package add

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Added package '{name}'` |
| Package already in homeos.yml (error) | stderr | `Error: Package '{name}' already exists` |
| Package directory already exists (error) | stderr | `Error: Package directory '{name}' already exists. Remove it first to re-create.` |
| Plugin not found (error) | stderr | `Error: Plugin '{name}' not found. Add it first with: homeos plugin add {name}` |
| Missing plugin params (error) | stderr | `Error: Missing required plugin parameters: {params}` |
| Invalid key=value pair (error) | stderr | `Error: invalid key=value pair: no '=' found in '{input}'` |

## homeos package remove

| Condition | Dest | Output |
|-----------|------|--------|
| Confirmation prompt | stdout | `The following packages will be removed from homeos.yml:` / `  {name}` |
| With --purge | stdout | `The following directories will be deleted:` / `  {path}` |
| User declines | stdout | `Aborted.` |
| Success | stdout | `Removed package '{name}'` |
| Success with --purge | stdout | `Removed package '{name}' and deleted directory` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Package is installed (error) | stderr | `Error: Package '{name}' is currently installed. Uninstall it first with: homeos package uninstall {name}` |
| Package is depended on (error) | stderr | `Error: Cannot remove package '{name}' because it is depended on by: {dependents}` |

## homeos package add-dep

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Added dependency '{dependency}' to package '{name}'` |
| Already depends | stdout | `Package '{name}' already depends on '{dependency}'` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |

## homeos package remove-dep

| Condition | Dest | Output |
|-----------|------|--------|
| Success | stdout | `Removed dependency '{dependency}' from package '{name}'` |
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
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Installing {name}...` / `done` or `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Circular dependency (error) | stderr | `Error: Circular dependency detected among packages: {names}` |
| Some packages fail | stdout | `Some packages failed` |

## homeos package update

| Condition | Dest | Output |
|-----------|------|--------|
| Plan display | stdout | (see Plan Display section below) |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Updating {name}...` / `done` or `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Some packages fail | stdout | `Some packages failed` |

## homeos package uninstall

| Condition | Dest | Output |
|-----------|------|--------|
| Plan display | stdout | (see Plan Display section below) |
| User declines | stdout | `Aborted.` |
| Script execution | stdout | `Uninstalling {name}...` / `done` or `FAILED` |
| Script not found (error) | stdout | `Error: Script not found: {path}` |
| Script execution fails (error) | stdout | `Error: Script failed with exit code {code}` |
| Package not found (error) | stderr | `Error: Package '{name}' not found` |
| Circular dependency (error) | stderr | `Error: Circular dependency detected among packages: {names}` |
| Some packages fail | stdout | `Some packages failed` |

## homeos plugin list

| Condition | Dest | Output |
|-----------|------|--------|
| No plugins | stdout | `No plugins.` |
| Has plugins | stdout | Table: `Name`, `URL` columns |

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
| Warning about referencing packages | stdout | `Warning: the following packages reference plugin '{name}': {packages}` |
| Confirmation prompt | stdout | `The following plugins will be removed from homeos.yml:` / `  {name}` |
| With --purge | stdout | `The following directories will be deleted:` / `  {path}` |
| User declines | stdout | `Aborted.` |
| Success | stdout | `Removed plugin '{name}'` |
| Success with --purge | stdout | `Removed plugin '{name}' and deleted directory` |
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
| No repositories | stdout | `No repositories.` |
| Has repositories | stdout | One repository name per line |

## homeos repo add

| Condition | Dest | Output |
|-----------|------|--------|
| Clone success | stdout | `Repository '{name}' cloned successfully` |
| Create success | stdout | `Repository '{name}' created` |
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
The following packages will be skipped:
  {name} (disabled)
  {name} (already installed)
  {name} (not installed)

Proceed? [y/N]
```

When all packages are skipped (no confirmation prompt):

```
The following packages will be skipped:
  {name} (disabled)
  {name} (already installed)

Nothing to do.
```

# Command Output Specification

This file defines all user-facing output for homeos commands. When adding or changing messages, follow this specification to maintain consistency.

Errors are displayed via `eprintln!("Error: {e}");` in main.rs. In the tables below, conditions marked with `(error)` are sent to stderr with the `Error:` prefix automatically.

## homeos init

| Condition | Output |
|-----------|--------|
| Already initialized | `Already initialized at {path}` |
| Scaffold success | `Initialized homeos at {path}` |
| Clone success | `Initialized homeos at {path} (cloned from {url})` |
| git clone fails (error) | `git clone failed: {stderr}` |
| Cloned repo has no homeos.yml (error) | `Not a valid homeos repository` |

## homeos cd

| Condition | Output |
|-----------|--------|
| Repos directory not found (error) | `Repos directory not found at {path}. Run 'homeos init' first.` |

## homeos apply

| Condition | Output |
|-----------|--------|
| Nothing to do | `Nothing to do.` |
| Disabled packages skipped | `The following packages will be skipped:` / `  {name} (disabled)` |
| Plan display | (see Plan Display section below) |
| User confirms | Executes with progress messages |
| User declines | `Aborted.` |
| Script not found (error) | `Script not found: {path}` |
| Script execution | `Installing {name}...` / `done` or `FAILED` |
| Some packages fail (error) | `Some packages failed` |

## homeos package list

| Condition | Output |
|-----------|--------|
| No packages | `No packages.` |
| Has packages | Table: `Package`, `Enabled`, `Installed` columns |

## homeos package add

| Condition | Output |
|-----------|--------|
| Success | `Added package '{name}'` |
| Package already in homeos.yml (error) | `Package '{name}' already exists` |
| Package directory already exists (error) | `Package directory '{name}' already exists. Remove it first to re-create.` |
| Plugin not found (error) | `Plugin '{name}' not found. Add it first with: homeos plugin add {name}` |
| Missing plugin params (error) | `Missing required plugin parameters: {params}` |
| Invalid key=value pair (error) | `invalid key=value pair: no '=' found in '{input}'` |

## homeos package remove

| Condition | Output |
|-----------|--------|
| Confirmation prompt | `The following packages will be removed from homeos.yml:` / `  {name}` |
| With --purge | `The following directories will be deleted:` / `  {path}` |
| User declines | `Aborted.` |
| Success | `Removed package '{name}'` |
| Success with --purge | `Removed package '{name}' and deleted directory` |
| Package not found (error) | `Package '{name}' not found` |
| Package is installed (error) | `Package '{name}' is currently installed. Uninstall it first with: homeos package uninstall {name}` |
| Package is depended on (error) | `Cannot remove package '{name}' because it is depended on by: {dependents}` |

## homeos package add-dep

| Condition | Output |
|-----------|--------|
| Success | `Added dependency '{dependency}' to package '{name}'` |
| Already depends | `Package '{name}' already depends on '{dependency}'` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package remove-dep

| Condition | Output |
|-----------|--------|
| Success | `Removed dependency '{dependency}' from package '{name}'` |
| Not a dependency | `Package '{name}' does not depend on '{dependency}'` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package add-alias

| Condition | Output |
|-----------|--------|
| Success | `Added alias '{target}={source}' to package '{name}'` |
| Already has alias | `Package '{name}' already has alias '{target}'` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package remove-alias

| Condition | Output |
|-----------|--------|
| Success | `Removed alias '{target}' from package '{name}'` |
| Alias not found | `Package '{name}' does not have alias '{target}'` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package enable

| Condition | Output |
|-----------|--------|
| Success | `Enabled package '{name}'` |
| Already enabled | `Package '{name}' is already enabled` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package disable

| Condition | Output |
|-----------|--------|
| Success | `Disabled package '{name}'` |
| Already disabled | `Package '{name}' is already disabled` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package cat

| Condition | Output |
|-----------|--------|
| Script exists | `=== {filename} ===` / `{content}` |
| Script not found | `=== {filename} ===` / `(not found)` |
| Package not found (error) | `Package '{name}' not found` |

## homeos package cd

| Condition | Output |
|-----------|--------|
| Package not found (error) | `Package '{name}' not found` |
| Directory not found (error) | `Directory not found at {path}` |

## homeos package install

| Condition | Output |
|-----------|--------|
| Plan display | (see Plan Display section below) |
| No packages to install | `No packages to install.` |
| User declines | `Aborted.` |
| Script execution | `Installing {name}...` / `done` or `FAILED` |
| Package not found (error) | `Package '{name}' not found` |
| Circular dependency (error) | `Circular dependency detected among packages: {names}` |
| Script fails (error) | `Script failed with exit code {code}` |
| Some packages fail (error) | `Some packages failed` |

## homeos package update

| Condition | Output |
|-----------|--------|
| Plan display | (see Plan Display section below) |
| No packages to update | `No packages to update.` |
| User declines | `Aborted.` |
| Script execution | `Updating {name}...` / `done` or `FAILED` |
| Package not found (error) | `Package '{name}' not found` |
| Script fails (error) | `Script failed with exit code {code}` |
| Some packages fail (error) | `Some packages failed` |

## homeos package uninstall

| Condition | Output |
|-----------|--------|
| Plan display | (see Plan Display section below) |
| No packages to uninstall | `No packages to uninstall.` |
| User declines | `Aborted.` |
| Script execution | `Uninstalling {name}...` / `done` or `FAILED` |
| Package not found (error) | `Package '{name}' not found` |
| Circular dependency (error) | `Circular dependency detected among packages: {names}` |
| Script fails (error) | `Script failed with exit code {code}` |
| Some packages fail (error) | `Some packages failed` |

## homeos plugin list

| Condition | Output |
|-----------|--------|
| No plugins | `No plugins.` |
| Has plugins | Table: `Name`, `URL` columns |

## homeos plugin list-remote

| Condition | Output |
|-----------|--------|
| No remote plugins | `No remote plugins found.` |
| Has plugins | Table: `Name`, `Description`, `URL` columns |

## homeos plugin add

| Condition | Output |
|-----------|--------|
| Local success | `Plugin '{name}' created locally` |
| Clone success | `Plugin '{name}' added successfully` |
| Plugin not found on GitHub (error) | `Plugin '{name}' not found on GitHub (homeos-plugin-{name})` |
| Plugin already in homeos.yml (error) | `Plugin '{name}' already exists` |
| Plugin directory already exists (error) | `Plugin directory '{name}' already exists` |
| git clone fails (error) | `git clone failed: {stderr}` |
| Cloned plugin has no plugin.yml (error) | `Not a valid homeos plugin` |

## homeos plugin remove

| Condition | Output |
|-----------|--------|
| Warning about referencing packages | `Warning: the following packages reference plugin '{name}': {packages}` |
| Confirmation prompt | `The following plugins will be removed from homeos.yml:` / `  {name}` |
| With --purge | `The following directories will be deleted:` / `  {path}` |
| User declines | `Aborted.` |
| Success | `Removed plugin '{name}'` |
| Success with --purge | `Removed plugin '{name}' and deleted directory` |
| Plugin not found (error) | `Plugin '{name}' not found` |

## homeos plugin cat

| Condition | Output |
|-----------|--------|
| plugin.yml exists | `=== plugin.yml ===` / `{content}` |
| plugin.yml not found | `=== plugin.yml ===` / `(not found)` |
| Template exists | `=== {filename} ===` / `{content}` |
| Template not found | `=== {filename} ===` / `(not found)` |
| Plugin not found (error) | `Plugin '{name}' not found` |

## homeos plugin cd

| Condition | Output |
|-----------|--------|
| Plugin not found (error) | `Plugin '{name}' not found` |
| Directory not found (error) | `Directory not found at {path}` |

## homeos repo list

| Condition | Output |
|-----------|--------|
| No repositories | `No repositories.` |
| Has repositories | One repository name per line |

## homeos repo add

| Condition | Output |
|-----------|--------|
| Clone success | `Repository '{name}' cloned successfully` |
| Create success | `Repository '{name}' created` |
| Repository already exists (error) | `Repository '{name}' already exists` |
| git clone fails (error) | `git clone failed: {stderr}` |

## homeos repo cd

| Condition | Output |
|-----------|--------|
| Repository not found (error) | `Repository '{name}' does not exist` |

## homeos repo remove

| Condition | Output |
|-----------|--------|
| Confirmation prompt | `Remove repository '{name}'?` |
| User declines | `Aborted.` |
| Success | `Repository '{name}' removed` |
| Removing default (error) | `Cannot remove the default repository.` |
| Repository not found (error) | `Repository '{name}' does not exist` |
| Has installed packages (error) | `Repository '{name}' has installed packages. Uninstall them first.` |

## Plan Display

Used by `apply`, `install`, `update`, `uninstall`:

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

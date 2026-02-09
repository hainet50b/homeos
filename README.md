# homeos

> Pronounced /ˈhoʊ.mi.oʊs/ (ho-mee-os).  
> ⚠️ This project is under active development.

## Table of contents

- [Quick Start](#quick-start)
- [Commands](#commands)
  - [General](#general)
  - [Manage packages](#manage-packages)
  - [Operate packages](#operate-packages)
  - [Manage recipes](#manage-recipes)
  - [Execute recipes](#execute-recipes)
  - [Manage repositories](#manage-repositories)
  - [Manage plugins](#manage-plugins)
  - [Options](#options)
- [Directory Structure](#directory-structure)
- [Configuration File (homeos.yml)](#configuration-file-homeosyml)
  - [Defaults](#defaults)
  - [Platforms](#platforms)
  - [Profiles](#profiles)
  - [Packages](#packages)
  - [Recipes](#recipes)
  - [Plugins](#plugins)
- [Repository Design Patterns](#repository-design-patterns)

## Quick Start

> ⚠️ Under construction

### Using a starter repository

`homeos` itself does not provide built-in templates.  
Instead, you can initialize your workspace from an external starter repository.

If you want to use a boilerplate repository without inheriting its Git history, use the `--strip-git` option with the `init` subcommand.  
This option removes the `.git` directory after cloning the remote repository, allowing you to start with a clean repository.

**Example**

```sh
homeos init default https://github.com/hainet50b/homeos-starter --strip-git
homeos cd
git init
```

## Commands

### General

```sh
homeos init [<repo_name> <repo_url>]
homeos cd
homeos apply
```

- `init`  
  Create the initial repository structure under `~/.local/share/homeos/repos`.  
  If `<repo_name>` and `<repo_url>` are given, clone the remote repository as the default repo.
- `cd`  
  Move to the default repository directory.
- `apply`  
  Install missing packages, update installed ones, execute any unapplied recipes, and add defined plugins.

#### Flags

- `--strip-git`  
  If this flag is given, clone the remote repository without inheriting its Git history.

### Manage packages

```sh
homeos package list
homeos package add <package>
homeos package remove <package>
homeos package cd [<package>]
```

Manage package definitions inside the repository.

- `list`  
  List all defined packages in the repository.
- `add`  
  Create a new package directory under `packages/` and update `homeos.yml`.
- `remove`  
  Remove the package entry from `homeos.yml`.
- `cd`  
  Move to the package root directory.  
  If name is given, move to the package directory.

#### Flags

- `--installed`  
  If this flag is given, list installed package only.

### Operate packages

```sh
homeos package install [<package>]
homeos package update [<package>]
homeos package uninstall [<package>]
homeos package enable <package>
homeos package disable <package>
```

Operate on packages defined in the current repository.

- `install`  
  Execute the install action for all applicable packages.  
  If no package is specified, operate on the package.
- `update`  
  Execute the update action for the specified package.
- `uninstall`  
  Execute the uninstall action for the specified package.
- `enable`  
  Mark the specified package as enabled in `homeos.yml`.  
  This removes `enabled: false` so the package will be included in `homeos apply`.
- `disable`  
  Mark the specified package as disabled in `homeos.yml`.  
  This sets `enabled: false` so the package will be skipped by `homeos apply` and shown as disabled in `homeos list`.

The actual script executed is determined by in `homeos.yml`:

- defaults
- platform
- profile
- packages

### Manage recipes

```sh
homeos recipe list
homeos recipe add <recipe>
homeos recipe remove <recipe>
homeos recipe cd [<recipe>]
```

Recipes are ordered script collections used for grouped operations.

- `list`  
  Lis all available recipes in the repository.
- `add`  
  Create a new recipe directory under `recipes/` and update `homeos.yml`.
- `remove`  
  Remove the recipe entry and directory.
- `cd`  
  Move to the recipe root directory.  
  If name is given, move to the recipe directory.

### Execute recipes

```sh
homeos recipe exec <recipe>
homeos recipe exec <recipe>/<script>
```

- `exec`  
  Execute all scripts in the recipe in order.
- `exec <recipe>/<script>`  
  Execute a specific script within the recipe.

### Manage repositories

```sh
homeos repo list
homeos repo add <repo_name> <repo_url>
homeos repo remove <repo_name>
```

Manage additional repositories alongside the default repository.

- `list`  
  List registered repositories.
- `add`  
  Clone a remote repository into `repos/<repo_name>/`.
- `remove`  
  Remove the local repository directory.

Each repository contains its own `homeos.yml`.

### Manage plugins

```sh
homeos plugin list
homeos plugin add <plugin_name> <plugin_url>
homeos plugin remove <plugin_name>
```

Manage plugins used to provide package action implementations.

- `list`  
  List registered plugins in the current repository.
- `add`  
  Register a plugin by name and clone it from the given URL.
- `remove`  
  Remove the plugin directory and entry from `homeos.yml`.

Each repository manages its own plugins.

### Options

```
--repo, -r        Specify repository
--profile, -p     Specify profile
--platform, -P    Specify platform
```

- `--repo`  
  Select which repository to operate on.  
  Defaults to `default`.
- `--profile`  
  Select which profile to use when resolving packages and actions.  
  Default profile is determined by `homeos.yml`.
- `--platform`  
  Select which platform to use when resolving actions.  
  Default platform is determined by `homeos.yml`.

Profiles affect:

- tag filtering
- action selection

## Directory Structure

```
~/
└── .local/
    └── share/
        └── homeos/
            └── repos/
                ├── default/
                │   ├── homeos.yml
                │   ├── packages/
                │   │   └── neovim/
                │   │       ├── install.sh
                │   │       ├── update.sh
                │   │       └── uninstall.sh
                │   ├── recipes/
                │   │   └── my-recipe/
                │   │       ├── 01-foo.sh
                │   │       └── 02.bar.sh
                │   └── plugins/
                │       └── dnf/
                ├── remote-repo1
                └── remote-repo2
```

- Each repository is self-contained.
- `homeos.yml` defines behavior and package metadata.
- `packages/` contains package action scripts.
- `recipes/` contains grouped execution flows.
- `plugins/` contains plugin implementations.

## Configuration File (homeos.yml)

### Defaults

```yaml
defaults:
  actions: { install: install.sh, update: update.sh, uninstall: uninstall.sh }
  profile: home-desktop
  platform: linux
```

Global defaults used when no overrides are present.

- `actions`  
  Default script names for each action.
- `profile`  
  Default profile used when `--profile` is not specified.
- `platform` (Optional)  
  The platform identifier used to resolve platform-specific overrides.

### Platforms

```yaml
platforms:
  linux:
    actions_overrides: { install: install.ps1, update: update.ps1, uninstall: uninstall.ps1 }
```

Platform-specific befavior overrides.

- `actions_overrides`  
  Replace default action scripts for the given platform.

### Profiles

```yaml
profiles:
  home-desktop:
    tags_any: [ cli, desktop ]
    tags_all: [ home ]
```

Profiles control which packages are active.

- `tags_any`  
  Packages matching any of the specified tags.
- `tags_all`  
  Packages must contain all specified tags.

### Packages

```yaml
packages:
  neovim:
    tags: [ cli, home, work ]
    actions_overrides: { update: install }
    enabled: false
```

Package-specific metadata.

- `tags` (optional)  
  Used for profile-based selection.
- `actions_overrides` (optional)  
  Overrides the default action mapping for this package.  
  `install` / `update` / `uninstall` can be used for the alias for related action.
- `enabled` (optional, default: true)  
  Controls whether this package is managed by homeos.  
  When set to `false`, the package is ignored by `homeos apply`.

### Recipes

```yaml
recipes:
  my-recipe:
    tags: [ desktop, home ]
```

- `tags`  
  Used for profile-based selection.

### Plugins

```yaml
plugins:
  dnf:
    url: https://github.com/hainet50b/homeos-plugin-dnf
```

Plugins provide implementations for package actions such as `install`, `update`, and `uninstall`.

- Each plugin is identified by a unique name.
- The `url` specifies the remote repository used to obtain the plugin.

Packages can reference a plugin to delegate actions.  
When a package specifies a plugin, its action behavior is resolved through that plugin.

```yaml
packages:
  neovim:
    plugin: dnf
    params:
      name: neovim.x86_64
```

- `params` schema is defined by a plugin.
- Package-level `actions_overrides` takes precedence over plugin-provided actions.

## Repository design patterns

There are two common ways to design your `homeos` repositories.

### Pattern 1: Single repository (recommended for new users)

Manage packages, recipes, and multiple plugins in a single repository.  
This pattern keeps your workspace simple and is a good default for personal setups.

- Use `platform` / `profile` to select actions and packages.
- Register multiple plugins under `plugins/` as needed (e.g. `dnf` / `winget`).

This is the recommended starting point.

### Pattern 2: Multiple repositories by provider / platform

Split repositories by package provider or platform, and use them side by side.

Examples:

- `homeos-repo-linux` (shell script and dnf/apt-based packages)
- `homeos-repo-windows` (winget/scoop/chocolatey-based packages)

This pattern can be useful when:

- you maintain both Linux and Windows environments,
- you want stricter separation of responsibilities,
- or you share repositories across teams.

`homeos` supports both patterns. Start with Pattern 1, and split repositories only when it becomes beneficial.

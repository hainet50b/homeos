# homeos

> Pronounced /ˈhoʊ.mi.oʊs/ (ho-mee-os).  
> ⚠️ This project is under active development.

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
```

- `init`  
  Create the initial repository structure under `~/.local/share/homeos/repos`.  
  If `<repo_name>` and `<repo_url>` are given, clone the remote repository as the default repo.
- `cd`  
  Move to the default repository directory.

#### Flags

- `--strip-git`  
  If this flag is given, clone the remote repository without inheriting its Git history.

### Install packages

```sh
homeos list
homeos install [<package>]
homeos update [<package>]
homeos remove [<package>]
```

Operate on packages defined in the current repository.

- `list`  
  List all available packages across all registered repositories.
- `install`  
  Execute the install action for all applicable packages.  
  If no package is specified, operate on the package.
- `update`  
  Execute the update action for the specified package.
- `remove`  
  Execute the remove action for the specified package.

The actual script executed is determined by in `homeos.yml`:

- defaults
- platform
- profile
- package overrides

#### Flags

- `--installed`  
  If this flag is given, list installed package only.

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

### Manage recipes

```sh
homeos recipe list
homeos recipe add <recipe>
homeos recipe remove <recipe>
homeos recipe cd [<recipe>]
homeos recipe exec <recipe>
homeos recipe exec <recipe>/<script>
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
- `exec`  
  Execute all scripts in the recipe in order.
- `exec <recipe>/<script>`  
  Execute a specific script within the recipe.

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
                │   │       └── remove.sh
                │   └── recipes/
                │       └── my-recipe/
                │           ├── 01-foo.sh
                │           └── 02.bar.sh
                ├── remote-repo1
                └── remote-repo2
```

- Each repository is self-contained.
- `homeos.yml` defines behavior and package metadata.
- `packages/` contains package action scripts.
- `recipes/` contains grouped execution flows.

## Configuration File (homeos.yml)

### Defaults

```yaml
defaults:
  actions: { install: install.sh, update: update.sh, remove: remove.sh }
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
    actions_overrides: { install: install.ps1, update: update.ps1, remove: remove.ps1 }
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
```

Package-specific metadata.

- `tags`  
  Used for profile-based selection.
- `actions_overrides`  
  Override the default action mapping for this package.  
  `install` / `update` / `remove` is used for the alias for related action.


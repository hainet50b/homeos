# homeos

<!-- TODO: badges (build status, release version, license) -->

**Making install script hell feel like *home*.**  
⚠️ This project is under active development.

*homeos* (ho-mee-os) — named after *homeostasis*, a layer above your install scripts, managed from a single Git repository.

## Features

- **Source of truth in a single Git repo** — Everything in your *home* is visible and under your control. Nothing happens beyond what you write.
- **One interface, any provider** — Manage custom scripts and package providers alike through one interface and one config file.
- **Install in the right order** — Define dependencies between packages. homeos respects them in every operation, executing scripts in the correct order.
- **Nothing runs without confirmation** — A plan is always shown before execution. If it's not in the plan, it doesn't run.
- **Run anywhere: Linux, macOS, Windows** — Built with Rust. Works on any OS.

[Quick Tour](#quick-tour) | [Install](#install) | [Reference](#reference) | [Official Plugins](#official-plugins) | [Plugin Development Guide](#plugin-development-guide)

## Quick Tour

1. Initialize a new repository

```sh
$ homeos init
TODO
```

2. Add a package

```sh
$ homeos package add rustup
TODO
```

3. Move to a package directory and edit its install scripts

```sh
# Open a new shell in the package directory
$ homeos package cd rustup
$ ls
TODO

# Edit your scripts here

# Return to the previous shell
$ exit
```

4. Verify the scripts

```sh
$ homeos package cat rustup
TODO
```

5. Apply, review the plan and install

```sh
$ homeos apply
TODO

$ rustup --version
TODO
```

<details>
<summary>Using a plugin</summary>

```sh
# 1. Browse official plugins
# 2. Add a plugin and create a package with it
# 3. Apply
```

</details>

<details>
<summary>Setting up a new machine from your repo</summary>

```sh
# 1. Clone your existing homeos repository
# 2. Apply — everything gets installed
```

</details>

## Install

Download the prebuilt binary from [GitHub Releases](https://github.com/hainet50b/homeos/releases).

## Reference

### Directory Structure

The base directory depends on the operating system:

| OS      | Base directory                         |
|---------|----------------------------------------|
| Linux   | `~/.local/share/homeos`                |
| macOS   | `~/Library/Application Support/homeos` |
| Windows | `%LOCALAPPDATA%/homeos`                |

```
<base_dir>/repos/
├── default/
│   ├── homeos.yml
│   ├── state.yml
│   ├── .gitignore
│   ├── packages/
│   │   └── neovim/
│   │       ├── install.sh
│   │       ├── update.sh
│   │       └── uninstall.sh
│   └── plugins/
│       └── dnf/
└── other-repo/
```

- `homeos.yml` — package and plugin definitions.
- `state.yml` — tracks installed packages. Machine-specific, excluded from version control via `.gitignore`.
- `packages/` — action scripts (`install.sh`, `update.sh`, `uninstall.sh` for Linux/macOS; `.ps1` for Windows).
- `plugins/` — plugin files.

### Configuration (homeos.yml)

```yaml
packages:
  claude:
    depends_on: [bubblewrap, sandbox-runtime, socat]
    script_aliases: { update: install }
  bubblewrap:
    plugin: dnf
    params:
      name: bubblewrap
  sandbox-runtime:
    plugin: npm
    params:
      name: "@anthropic-ai/claude-code-sandbox-runtime"
  socat:
    plugin: dnf
    params:
      name: socat
  ollama:
    script_aliases: { update: install }
    enabled: false

plugins:
  dnf:
    url: https://github.com/hainet50b/homeos-plugin-dnf
  npm:
    url: https://github.com/hainet50b/homeos-plugin-npm
```

- `depends_on` — declare dependencies on other packages.
- `script_aliases` — map an action to another script (e.g., run `install.sh` for `update`).
- `enabled` — `true` by default. Set to `false` to skip the package in operations.
- `plugin` — plugin name to use for script generation.
- `params` — values passed to the plugin's templates.

### Core Commands

#### `homeos init`

Create the initial repo structure. Without arguments, creates an empty repository with a skeleton `homeos.yml`. With a URL, clones the remote repository as the default repo — use `--repo` to clone it as a named repo.

```
Usage: homeos init [OPTIONS] [URL]

Arguments:
  [URL]  Remote URL to clone as the default repo

Options:
      --strip-git  Remove .git directory after cloning
```

#### `homeos cd`

Launch a shell in the repo directory.

```
Usage: homeos cd
```

#### `homeos apply`

Install new packages and update installed ones.

```
Usage: homeos apply
```

A confirmation prompt is shown before execution.

```
$ homeos apply
The following packages will be installed:
  bubblewrap
  socat
  claude
The following packages will be updated:
  neovim
The following packages will be skipped:
  zed (disabled)

Proceed? [y/N]
```

> [!NOTE]
> Packages are installed in dependency order based on `depends_on`.

### Manage packages

Packages can be **enabled** or **disabled**. Disabled packages are skipped by `apply`, `install`, and `update`. `uninstall` runs regardless of the enabled status. Newly added packages are enabled by default.

Packages can declare **dependencies** on other packages. homeos handles them in the right order.

#### `homeos package list`

List all packages.

```
Usage: homeos package list
```

Displays a table with package name, enabled status, and installed status.

```
$ homeos package list
Package     Enabled   Installed
neovim      yes       yes
zed         yes       no
docker      no        no
```

#### `homeos package add`

Add a new package. Creates a package directory under `packages/` and adds an entry to `homeos.yml`. Skeleton scripts are generated with OS-appropriate extensions (`.sh` for Linux/macOS, `.ps1` for Windows).

```
Usage: homeos package add [OPTIONS] <PACKAGE>

Arguments:
  <PACKAGE>  Package name

Options:
      --depends-on <DEPENDS_ON>...        Dependencies for this package
      --script-aliases <ALIAS>...           Script aliases as target=source pairs (e.g., update=install)
      --plugin <PLUGIN>                   Plugin to use for generating scripts
      --params <PARAMS>...                Plugin parameters as key=value pairs
```

To generate scripts from a plugin instead of default skeletons:

```
$ homeos package add neovim --plugin dnf --params name=neovim.x86_64
```

#### `homeos package remove`

Remove package entries from `homeos.yml`. The package directory is not deleted.

```
Usage: homeos package remove <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package add-dep`

Add dependencies to an existing package.

```
Usage: homeos package add-dep <PACKAGE> <DEPENDENCY>...

Arguments:
  <PACKAGE>        Package name
  <DEPENDENCY>...  Dependencies to add
```

#### `homeos package remove-dep`

Remove dependencies from an existing package.

```
Usage: homeos package remove-dep <PACKAGE> <DEPENDENCY>...

Arguments:
  <PACKAGE>        Package name
  <DEPENDENCY>...  Dependencies to remove
```

#### `homeos package add-alias`

Add script aliases to an existing package.

```
Usage: homeos package add-alias <PACKAGE> <ALIAS>...

Arguments:
  <PACKAGE>    Package name
  <ALIAS>...   Aliases as target=source pairs (e.g., update=install)
```

#### `homeos package remove-alias`

Remove script aliases from a package.

```
Usage: homeos package remove-alias <PACKAGE> <ALIAS>...

Arguments:
  <PACKAGE>    Package name
  <ALIAS>...   Alias targets to remove (e.g., update)
```

#### `homeos package enable`

Enable packages.

```
Usage: homeos package enable <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package disable`

Disable packages.

```
Usage: homeos package disable <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package cat`

Display all scripts for a package.

```
Usage: homeos package cat <PACKAGE>

Arguments:
  <PACKAGE>  Package name
```

If a script does not exist, `(not found)` is shown.

```
$ homeos package cat rustup
=== install.sh ===
#!/usr/bin/env sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

=== update.sh ===
#!/usr/bin/env sh
rustup update

=== uninstall.sh ===
(not found)
```

#### `homeos package cd`

Launch a shell in the package root or specific package directory.

```
Usage: homeos package cd [PACKAGE]

Arguments:
  [PACKAGE]  Package name (optional — defaults to packages root)
```

### Operate packages

homeos tracks which packages are installed on this machine in `state.yml`. This file is machine-specific and managed automatically — you don't need to edit it. The behavior of `install`, `update`, and `uninstall` depends on the package's enabled status and whether it is in `state.yml`.

A confirmation prompt is shown before execution. On script failure, the error is reported and execution continues to the next package.

| State                       | install                  | update                  | uninstall               |
|-----------------------------|--------------------------|-------------------------|-------------------------|
| enabled + not in state      | Execute                  | Skip (not installed)    | Skip (not installed)    |
| enabled + in state          | Skip (already installed) | Execute                 | Execute                 |
| disabled + not in state     | Skip (disabled)          | Skip (disabled)         | Skip (not installed)    |
| disabled + in state         | Skip (disabled)          | Skip (disabled)         | Execute                 |

> [!NOTE]
> `uninstall` ignores the enabled/disabled status — its only concern is whether the package is in `state.yml`. After a successful uninstall, the package is automatically disabled in `homeos.yml`.

#### `homeos package install`

Execute install scripts. Records installed packages in `state.yml`.

```
Usage: homeos package install <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package update`

Execute update scripts.

```
Usage: homeos package update <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package uninstall`

Execute uninstall scripts. Disables packages in `homeos.yml` and removes them from `state.yml`. The package directory is not deleted.

```
Usage: homeos package uninstall [OPTIONS] [PACKAGES]...

Arguments:
  [PACKAGES]...  Package names

Options:
      --all  Uninstall all installed packages (from state.yml)
```

### Manage plugins

#### `homeos plugin list`

List all plugins.

```
Usage: homeos plugin list
```

#### `homeos plugin list-remote`

List official plugins available from GitHub.

```
Usage: homeos plugin list-remote
```

Displays name, description, and URL for each official plugin.

```
$ homeos plugin list-remote
Name     Description                        URL
dnf      DNF package manager plugin         https://github.com/hainet50b/homeos-plugin-dnf
winget   Windows Package Manager plugin     https://github.com/hainet50b/homeos-plugin-winget
```

#### `homeos plugin add`

Add a plugin. Without a URL, resolves the official repository automatically. See [Plugin Development Guide](#plugin-development-guide) for `--local` usage.

```
Usage: homeos plugin add [OPTIONS] <PLUGIN> [URL]

Arguments:
  <PLUGIN>  Plugin name
  [URL]   Remote URL to clone (defaults to official repository)

Options:
      --local  Create an empty plugin skeleton for local development
```

#### `homeos plugin remove`

Remove a plugin.

```
Usage: homeos plugin remove <PLUGIN>

Arguments:
  <PLUGIN>  Plugin name
```

> [!NOTE]
> The plugin directory is not deleted. To update a plugin, run `remove`, then `plugin cd` to navigate to the plugins directory, delete the plugin directory, and `add` again.

#### `homeos plugin cat`

Display `plugin.yml` and all template files for a plugin.

```
Usage: homeos plugin cat <PLUGIN>

Arguments:
  <PLUGIN>  Plugin name
```

Example:

```
$ homeos plugin cat dnf
=== plugin.yml ===
params:
  - name

=== install.sh.tmpl ===
#!/usr/bin/env sh
sudo dnf install -y {{name}}

=== update.sh.tmpl ===
#!/usr/bin/env sh
sudo dnf install -y {{name}}

=== uninstall.sh.tmpl ===
#!/usr/bin/env sh
sudo dnf remove -y {{name}}
```

#### `homeos plugin cd`

Launch a shell in the plugins root or specific plugin directory.

```
Usage: homeos plugin cd [PLUGIN]

Arguments:
  [PLUGIN]  Plugin name (optional — defaults to plugins root)
```

### Manage repositories

homeos supports multiple repositories. Each repository contains its own `homeos.yml`, packages and plugins. Use `-r, --repo <name>` on any command to specify which repository to operate on (default: `default`).

#### `homeos repo list`

List all repositories.

```
Usage: homeos repo list
```

#### `homeos repo add`

Clone a remote repository.

```
Usage: homeos repo add <REPO> <URL>

Arguments:
  <REPO>  Repository name
  <URL>   Remote URL to clone
```

#### `homeos repo remove`

Delete a local repository.

```
Usage: homeos repo remove <REPO>

Arguments:
  <REPO>  Repository name
```

> [!NOTE]
> Fails if the repository's `state.yml` contains installed packages. Uninstall them first.

## Official Plugins

Official plugins are available. See each plugin's repository for details.

| Name | Description |
|------|-------------|
| [dnf](https://github.com/hainet50b/homeos-plugin-dnf) | DNF package manager plugin |
| [npm](https://github.com/hainet50b/homeos-plugin-npm) | npm global package plugin |
| [winget](https://github.com/hainet50b/homeos-plugin-winget) | Windows Package Manager plugin |

Built a community plugin? [Open an issue](https://github.com/hainet50b/homeos/issues/new) and we'll list it here.  
Want a plugin that doesn't exist yet? [Request it](https://github.com/hainet50b/homeos/issues/new) — we'd love to hear what you need.

## Plugin Development Guide

A plugin consists of a parameter list (`plugin.yml`) and template files (`*.tmpl`). When a user runs `homeos package add` with a plugin, the templates are rendered with the user's parameters.

Below is a step-by-step guide to creating a plugin.

### 1. Create a plugin

```sh
$ homeos plugin add my-provider --local
$ homeos plugin cd my-provider
$ ls
TODO: output
```

### 2. Define parameters

Edit `plugin.yml` to define the parameters that users must provide when using your plugin:

```yaml
params:
  - name
```

### 3. Write templates

Edit the template files for each action. `.sh.tmpl` files are for Linux/macOS, `.ps1.tmpl` files are for Windows. You can provide both, or only the ones relevant to your plugin's purpose — unused templates can be deleted.

Templates use `{{key}}` placeholders. When a user runs `homeos package add` with your plugin, each `{{key}}` is replaced with the corresponding value from the user's `params`.

For example, `install.sh.tmpl`:

```sh
#!/usr/bin/env sh
sudo dnf install -y {{name}}
```

When a user adds a package with `--params name=neovim.x86_64`:

```sh
$ homeos package add neovim --plugin my-provider --params name=neovim.x86_64

$ homeos package cat neovim
=== install.sh ===
#!/usr/bin/env sh
sudo dnf install -y neovim.x86_64
```

> [!TIP]
> Downloaded plugins can be freely edited locally. Changes only affect your machine and are not pushed upstream.

## License

> TODO

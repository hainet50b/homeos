use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::engine::ArgValueCompleter;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value pair: no '=' found in '{s}'"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

mod commands;
mod completers;
mod config;
mod context;
#[cfg(test)]
mod env_test;
mod error;
mod git;
mod output;
mod plan;
mod state;
mod topo;

use output::OutputFormat;

#[derive(Parser)]
#[command(
    name = "homeos",
    version,
    about = "Manage install scripts in one place, reproducible on any machine"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Override the data directory (defaults to OS data directory)
    #[arg(long, global = true, hide = true)]
    pub data_dir: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, value_enum, conflicts_with = "json")]
    pub output: Option<OutputFormat>,

    /// Shorthand for --output json
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create the initial repository structure
    Init {
        /// Remote URL to clone
        url: Option<String>,
        /// Remove .git directory after cloning
        #[arg(long, requires = "url")]
        strip_git: bool,
    },
    /// Launch a shell in the repositories directory
    Cd,
    /// Install new packages and update installed ones
    Apply {
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage packages
    Package {
        #[command(subcommand)]
        command: PackageCommands,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
    /// Print a shell completion script to stdout
    Completion {
        /// Target shell
        #[arg(value_enum)]
        shell: commands::completion::CompletionShell,
    },
}

#[derive(Subcommand)]
pub enum PluginCommands {
    /// List all plugins
    List,
    /// List official plugins available from GitHub
    ListRemote,
    /// Register a plugin
    Add {
        /// Plugin name
        plugin: String,
        /// Remote URL to clone (defaults to official repository)
        url: Option<String>,
        /// Create an empty plugin skeleton for local development
        #[arg(long)]
        local: bool,
    },
    /// Remove a plugin
    Remove {
        /// Plugin name
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: String,
        /// Also delete the plugin directory
        #[arg(long)]
        purge: bool,
    },
    /// Display plugin details
    Info {
        /// Plugin name
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: String,
    },
    /// Display plugin.yml and all template files for a plugin
    Cat {
        /// Plugin name
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: String,
    },
    /// Launch a shell in the plugins root or specific plugin directory
    Cd {
        /// Plugin name (optional — defaults to plugins root)
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PackageCommands {
    /// List all packages
    List,
    /// Add a new package
    Add {
        /// Package name
        package: String,
        /// Add a dependency (can be repeated)
        #[arg(
            long = "depends-on",
            action = clap::ArgAction::Append,
            add = ArgValueCompleter::new(completers::package_completer),
        )]
        depends_on: Vec<String>,
        /// Add a script alias as target=source (can be repeated)
        #[arg(long = "script-alias", action = clap::ArgAction::Append, value_parser = parse_key_value)]
        script_aliases: Vec<(String, String)>,
        /// Plugin to use for generating scripts
        #[arg(long, add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: Option<String>,
        /// Plugin parameter as key=value (can be repeated)
        #[arg(long = "param", action = clap::ArgAction::Append, value_parser = parse_key_value)]
        params: Vec<(String, String)>,
    },
    /// Remove package entries from homeos.yml
    Remove {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
        /// Also delete the package directory
        #[arg(long)]
        purge: bool,
    },
    /// Rename a package
    Rename {
        /// Current package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        old: String,
        /// New package name
        new: String,
    },
    /// Add dependencies to an existing package
    AddDep {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Dependencies to add
        #[arg(required = true)]
        dependency: Vec<String>,
    },
    /// Remove dependencies from an existing package
    RemoveDep {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Dependencies to remove
        #[arg(required = true)]
        dependency: Vec<String>,
    },
    /// Add script aliases to an existing package
    AddAlias {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Aliases as target=source pairs (e.g., update=install)
        #[arg(required = true, value_parser = parse_key_value)]
        alias: Vec<(String, String)>,
    },
    /// Remove script aliases from a package
    RemoveAlias {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Alias targets to remove (e.g., update)
        #[arg(required = true)]
        alias: Vec<String>,
    },
    /// Enable packages
    Enable {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
    },
    /// Disable packages
    Disable {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
    },
    /// Display package details
    Info {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
    },
    /// Display all scripts for a package
    Cat {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
    },
    /// Launch a shell in the package root or specific package directory
    Cd {
        /// Package name (optional — defaults to packages root)
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: Option<String>,
    },
    /// Execute install scripts
    Install {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute update scripts
    Update {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute uninstall scripts
    Uninstall {
        /// Package names
        #[arg(
            required_unless_present = "all",
            add = ArgValueCompleter::new(completers::package_completer),
        )]
        packages: Vec<String>,
        /// Uninstall all installed packages (from state.yml)
        #[arg(long)]
        all: bool,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
}

fn dispatch(ctx: &context::Context, command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Init { url, strip_git } => commands::init::run(ctx, url.as_deref(), strip_git),
        Commands::Cd => commands::cd::run(ctx),
        Commands::Apply { dry_run } => commands::package::apply(ctx, dry_run),
        Commands::Package { command } => match command {
            PackageCommands::List => commands::package::list(ctx),
            PackageCommands::Add {
                package,
                depends_on,
                script_aliases,
                plugin,
                params,
            } => {
                let script_aliases_map: BTreeMap<String, String> =
                    script_aliases.into_iter().collect();
                let params_map: BTreeMap<String, String> = params.into_iter().collect();
                commands::package::add(
                    ctx,
                    &package,
                    &depends_on,
                    &script_aliases_map,
                    plugin.as_deref(),
                    &params_map,
                )
            }
            PackageCommands::Remove { packages, purge } => {
                commands::package::remove(ctx, &packages, purge)
            }
            PackageCommands::Rename { old, new } => commands::package::rename(ctx, &old, &new),
            PackageCommands::AddDep {
                package,
                dependency,
            } => commands::package::add_dep(ctx, &package, &dependency),
            PackageCommands::RemoveDep {
                package,
                dependency,
            } => commands::package::remove_dep(ctx, &package, &dependency),
            PackageCommands::AddAlias { package, alias } => {
                commands::package::add_alias(ctx, &package, &alias)
            }
            PackageCommands::RemoveAlias { package, alias } => {
                commands::package::remove_alias(ctx, &package, &alias)
            }
            PackageCommands::Enable { packages } => commands::package::enable(ctx, &packages),
            PackageCommands::Disable { packages } => commands::package::disable(ctx, &packages),
            PackageCommands::Info { package } => commands::package::info(ctx, &package),
            PackageCommands::Cat { package } => commands::package::cat(ctx, &package),
            PackageCommands::Cd { package } => commands::package::cd(ctx, package.as_deref()),
            PackageCommands::Install { packages, dry_run } => {
                commands::package::install(ctx, &packages, dry_run)
            }
            PackageCommands::Update { packages, dry_run } => {
                commands::package::update(ctx, &packages, dry_run)
            }
            PackageCommands::Uninstall {
                packages,
                all,
                dry_run,
            } => commands::package::uninstall(ctx, &packages, all, dry_run),
        },
        Commands::Plugin { command } => match command {
            PluginCommands::List => commands::plugin::list(ctx),
            PluginCommands::ListRemote => commands::plugin::list_remote(ctx),
            PluginCommands::Add { plugin, url, local } => {
                commands::plugin::add(ctx, &plugin, url.as_deref(), local)
            }
            PluginCommands::Remove { plugin, purge } => {
                commands::plugin::remove(ctx, &plugin, purge)
            }
            PluginCommands::Info { plugin } => commands::plugin::info(ctx, &plugin),
            PluginCommands::Cat { plugin } => commands::plugin::cat(ctx, &plugin),
            PluginCommands::Cd { plugin } => commands::plugin::cd(ctx, plugin.as_deref()),
        },
        Commands::Completion { shell } => commands::completion::run(shell),
    }
}

fn main() {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();
    let output_format = OutputFormat::resolve(cli.output, cli.json);
    let ctx = context::Context::new(cli.data_dir).with_output_format(output_format);

    if let Err(e) = dispatch(&ctx, cli.command) {
        error::report(e.as_ref(), output_format);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_add_dep_help_shows_dependency_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let add_dep_cmd = package_cmd.find_subcommand("add-dep").unwrap();

        // Act
        let args: Vec<&str> = add_dep_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "dependency")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["dependency"]);
    }

    #[test]
    fn test_cli_version_matches_cargo_pkg_version() {
        // Arrange
        let cmd = Cli::command();

        // Act
        let version = cmd.get_version();

        // Assert
        assert_eq!(version, Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_cli_version_flag_long() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "--version"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected DisplayVersion error"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_cli_version_flag_short() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "-V"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected DisplayVersion error"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_remove_dep_help_shows_dependency_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let remove_dep_cmd = package_cmd.find_subcommand("remove-dep").unwrap();

        // Act
        let args: Vec<&str> = remove_dep_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "dependency")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["dependency"]);
    }

    #[test]
    fn test_add_alias_help_shows_alias_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let add_alias_cmd = package_cmd.find_subcommand("add-alias").unwrap();

        // Act
        let args: Vec<&str> = add_alias_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "alias")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["alias"]);
    }

    #[test]
    fn test_remove_alias_help_shows_alias_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let remove_alias_cmd = package_cmd.find_subcommand("remove-alias").unwrap();

        // Act
        let args: Vec<&str> = remove_alias_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "alias")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["alias"]);
    }

    #[test]
    fn test_plugin_add_local_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "add", "custom", "--local"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Add { plugin, url, local },
        } = cli.command
        {
            assert_eq!(plugin, "custom");
            assert!(url.is_none());
            assert!(local);
        } else {
            panic!("Expected PluginCommands::Add");
        }
    }

    #[test]
    fn test_plugin_add_without_local_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "add", "dnf"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Add { local, .. },
        } = cli.command
        {
            assert!(!local);
        } else {
            panic!("Expected PluginCommands::Add");
        }
    }

    #[test]
    fn test_add_plugin_option() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "add", "neovim", "--plugin", "dnf"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { plugin, .. },
        } = cli.command
        {
            assert_eq!(plugin, Some("dnf".to_string()));
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_script_alias_option() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--script-alias",
            "update=install",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { script_aliases, .. },
        } = cli.command
        {
            assert_eq!(
                script_aliases,
                vec![("update".to_string(), "install".to_string())]
            );
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_without_script_alias_defaults_to_empty() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "add", "neovim"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { script_aliases, .. },
        } = cli.command
        {
            assert!(script_aliases.is_empty());
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_param_option() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--plugin",
            "dnf",
            "--param",
            "name=neovim.x86_64",
            "--param",
            "repo=extra",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { params, .. },
        } = cli.command
        {
            assert_eq!(
                params,
                vec![
                    ("name".to_string(), "neovim.x86_64".to_string()),
                    ("repo".to_string(), "extra".to_string()),
                ]
            );
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_plugin_add_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let add_cmd = plugin_cmd.find_subcommand("add").unwrap();

        // Act
        let args: Vec<&str> = add_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_plugin_remove_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let remove_cmd = plugin_cmd.find_subcommand("remove").unwrap();

        // Act
        let args: Vec<&str> = remove_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_plugin_remove_purge_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "remove", "dnf", "--purge"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Remove { plugin, purge },
        } = cli.command
        {
            assert_eq!(plugin, "dnf");
            assert!(purge);
        } else {
            panic!("Expected PluginCommands::Remove");
        }
    }

    #[test]
    fn test_plugin_remove_without_purge_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "remove", "dnf"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Remove { purge, .. },
        } = cli.command
        {
            assert!(!purge);
        } else {
            panic!("Expected PluginCommands::Remove");
        }
    }

    #[test]
    fn test_plugin_cat_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let cat_cmd = plugin_cmd.find_subcommand("cat").unwrap();

        // Act
        let args: Vec<&str> = cat_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_plugin_cd_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let cd_cmd = plugin_cmd.find_subcommand("cd").unwrap();

        // Act
        let args: Vec<&str> = cd_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_add_without_plugin_defaults_to_none() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "add", "neovim"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { plugin, params, .. },
        } = cli.command
        {
            assert!(plugin.is_none());
            assert!(params.is_empty());
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_depends_on_does_not_consume_subsequent_options() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "claude",
            "--depends-on",
            "git",
            "--depends-on",
            "curl",
            "--plugin",
            "dnf",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add {
                depends_on, plugin, ..
            },
        } = cli.command
        {
            assert_eq!(depends_on, vec!["git", "curl"]);
            assert_eq!(plugin, Some("dnf".to_string()));
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_script_alias_can_be_repeated() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--script-alias",
            "update=install",
            "--script-alias",
            "uninstall=install",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { script_aliases, .. },
        } = cli.command
        {
            assert_eq!(
                script_aliases,
                vec![
                    ("update".to_string(), "install".to_string()),
                    ("uninstall".to_string(), "install".to_string()),
                ]
            );
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_param_does_not_consume_subsequent_options() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--plugin",
            "dnf",
            "--param",
            "name=neovim.x86_64",
            "--depends-on",
            "git",
            "--param",
            "repo=extra",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command:
                PackageCommands::Add {
                    depends_on,
                    params,
                    plugin,
                    ..
                },
        } = cli.command
        {
            assert_eq!(
                params,
                vec![
                    ("name".to_string(), "neovim.x86_64".to_string()),
                    ("repo".to_string(), "extra".to_string()),
                ]
            );
            assert_eq!(depends_on, vec!["git"]);
            assert_eq!(plugin, Some("dnf".to_string()));
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_package_remove_purge_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "remove", "neovim", "--purge"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Remove { packages, purge },
        } = cli.command
        {
            assert_eq!(packages, vec!["neovim".to_string()]);
            assert!(purge);
        } else {
            panic!("Expected PackageCommands::Remove");
        }
    }

    #[test]
    fn test_package_remove_without_purge_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "remove", "neovim"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Remove { purge, .. },
        } = cli.command
        {
            assert!(!purge);
        } else {
            panic!("Expected PackageCommands::Remove");
        }
    }

    #[test]
    fn test_apply_dry_run_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Apply { dry_run } = cli.command {
            assert!(dry_run);
        } else {
            panic!("Expected Commands::Apply");
        }
    }

    #[test]
    fn test_apply_without_dry_run_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply"]).unwrap();

        // Assert
        if let Commands::Apply { dry_run } = cli.command {
            assert!(!dry_run);
        } else {
            panic!("Expected Commands::Apply");
        }
    }

    #[test]
    fn test_package_install_dry_run_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "install", "neovim", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Install { dry_run, .. },
        } = cli.command
        {
            assert!(dry_run);
        } else {
            panic!("Expected PackageCommands::Install");
        }
    }

    #[test]
    fn test_package_update_dry_run_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "update", "neovim", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Update { dry_run, .. },
        } = cli.command
        {
            assert!(dry_run);
        } else {
            panic!("Expected PackageCommands::Update");
        }
    }

    #[test]
    fn test_package_uninstall_dry_run_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "uninstall", "neovim", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Uninstall { dry_run, .. },
        } = cli.command
        {
            assert!(dry_run);
        } else {
            panic!("Expected PackageCommands::Uninstall");
        }
    }

    #[test]
    fn test_output_flag_defaults_to_none() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply"]).unwrap();

        // Assert
        assert!(cli.output.is_none());
        assert!(!cli.json);
    }

    #[test]
    fn test_output_flag_parses_json() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--output", "json", "apply"]).unwrap();

        // Assert
        assert_eq!(cli.output, Some(OutputFormat::Json));
    }

    #[test]
    fn test_output_flag_parses_text() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--output", "text", "apply"]).unwrap();

        // Assert
        assert_eq!(cli.output, Some(OutputFormat::Text));
    }

    #[test]
    fn test_json_shorthand_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--json", "apply"]).unwrap();

        // Assert
        assert!(cli.json);
        assert!(cli.output.is_none());
    }

    #[test]
    fn test_output_and_json_flags_conflict() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "--output", "json", "--json", "apply"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected --output and --json to conflict"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_output_flag_is_global() {
        // Arrange & Act — accept after the subcommand
        let cli = Cli::try_parse_from(["homeos", "package", "list", "--output", "json"]).unwrap();

        // Assert
        assert_eq!(cli.output, Some(OutputFormat::Json));
    }

    #[test]
    fn test_json_flag_is_global() {
        // Arrange & Act — accept after the subcommand
        let cli = Cli::try_parse_from(["homeos", "package", "list", "--json"]).unwrap();

        // Assert
        assert!(cli.json);
    }
}

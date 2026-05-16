use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value pair: no '=' found in '{s}'"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

mod commands;
mod config;
mod context;
mod git;
mod plan;
mod state;
mod topo;

#[derive(Parser)]
#[command(
    name = "homeos",
    version,
    about = "Manage install scripts in one place, reproducible on any machine"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Override the base data directory (defaults to OS data directory)
    #[arg(long, global = true, hide = true)]
    pub base_dir: Option<PathBuf>,

    /// Specify repository
    #[arg(short = 'r', long, global = true, default_value = "default")]
    pub repo: String,
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
    /// Manage repositories
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
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
        plugin: String,
        /// Also delete the plugin directory
        #[arg(long)]
        purge: bool,
    },
    /// Display plugin details
    Info {
        /// Plugin name
        plugin: String,
    },
    /// Display plugin.yml and all template files for a plugin
    Cat {
        /// Plugin name
        plugin: String,
    },
    /// Launch a shell in the plugins root or specific plugin directory
    Cd {
        /// Plugin name (optional — defaults to plugins root)
        plugin: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum RepoCommands {
    /// List all repositories
    List,
    /// Add a repository
    Add {
        /// Repository name
        repo: String,
        /// Remote URL to clone
        url: Option<String>,
    },
    /// Launch a shell in the specified repository directory
    Cd {
        /// Repository name (default: "default")
        repo: Option<String>,
    },
    /// Delete a local repository
    Remove {
        /// Repository name
        repo: String,
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
        #[arg(long = "depends-on", action = clap::ArgAction::Append)]
        depends_on: Vec<String>,
        /// Add a script alias as target=source (can be repeated)
        #[arg(long = "script-alias", action = clap::ArgAction::Append, value_parser = parse_key_value)]
        script_aliases: Vec<(String, String)>,
        /// Plugin to use for generating scripts
        #[arg(long)]
        plugin: Option<String>,
        /// Plugin parameter as key=value (can be repeated)
        #[arg(long = "param", action = clap::ArgAction::Append, value_parser = parse_key_value)]
        params: Vec<(String, String)>,
    },
    /// Remove package entries from homeos.yml
    Remove {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
        /// Also delete the package directory
        #[arg(long)]
        purge: bool,
    },
    /// Rename a package
    Rename {
        /// Current package name
        old: String,
        /// New package name
        new: String,
    },
    /// Add dependencies to an existing package
    AddDep {
        /// Package name
        package: String,
        /// Dependencies to add
        #[arg(required = true)]
        dependency: Vec<String>,
    },
    /// Remove dependencies from an existing package
    RemoveDep {
        /// Package name
        package: String,
        /// Dependencies to remove
        #[arg(required = true)]
        dependency: Vec<String>,
    },
    /// Add script aliases to an existing package
    AddAlias {
        /// Package name
        package: String,
        /// Aliases as target=source pairs (e.g., update=install)
        #[arg(required = true, value_parser = parse_key_value)]
        alias: Vec<(String, String)>,
    },
    /// Remove script aliases from a package
    RemoveAlias {
        /// Package name
        package: String,
        /// Alias targets to remove (e.g., update)
        #[arg(required = true)]
        alias: Vec<String>,
    },
    /// Enable packages
    Enable {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Disable packages
    Disable {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Display package details
    Info {
        /// Package name
        package: String,
    },
    /// Display all scripts for a package
    Cat {
        /// Package name
        package: String,
    },
    /// Launch a shell in the package root or specific package directory
    Cd {
        /// Package name (optional — defaults to packages root)
        package: Option<String>,
    },
    /// Execute install scripts
    Install {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute update scripts
    Update {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute uninstall scripts
    Uninstall {
        /// Package names
        #[arg(required_unless_present = "all")]
        packages: Vec<String>,
        /// Uninstall all installed packages (from state.yml)
        #[arg(long)]
        all: bool,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let ctx = context::Context::new(cli.base_dir, cli.repo);

    match cli.command {
        Commands::Init { url, strip_git } => {
            if let Err(e) = commands::init::run(&ctx, url.as_deref(), strip_git) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Cd => {
            if let Err(e) = commands::cd::run(&ctx) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Apply { dry_run } => {
            if let Err(e) = commands::package::apply(&ctx, dry_run) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Plugin { command } => match command {
            PluginCommands::List => {
                if let Err(e) = commands::plugin::list(&ctx) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PluginCommands::ListRemote => {
                if let Err(e) = commands::plugin::list_remote() {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PluginCommands::Add { plugin, url, local } => {
                if let Err(e) = commands::plugin::add(&ctx, &plugin, url.as_deref(), local) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PluginCommands::Remove { plugin, purge } => {
                if let Err(e) = commands::plugin::remove(&ctx, &plugin, purge) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PluginCommands::Info { plugin } => {
                if let Err(e) = commands::plugin::info(&ctx, &plugin) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PluginCommands::Cat { plugin } => {
                if let Err(e) = commands::plugin::cat(&ctx, &plugin) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PluginCommands::Cd { plugin } => {
                if let Err(e) = commands::plugin::cd(&ctx, plugin.as_deref()) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        },
        Commands::Repo { command } => match command {
            RepoCommands::List => {
                if let Err(e) = commands::repo::list(&ctx) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            RepoCommands::Add { repo, url } => {
                if let Err(e) = commands::repo::add(&ctx, &repo, url.as_deref()) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            RepoCommands::Cd { repo } => {
                if let Err(e) = commands::repo::cd(&ctx, repo.as_deref()) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            RepoCommands::Remove { repo } => {
                if let Err(e) = commands::repo::remove(&ctx, &repo) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        },
        Commands::Completion { shell } => {
            if let Err(e) = commands::completion::run(shell) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Package { command } => match command {
            PackageCommands::List => {
                if let Err(e) = commands::package::list(&ctx) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
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
                if let Err(e) = commands::package::add(
                    &ctx,
                    &package,
                    &depends_on,
                    &script_aliases_map,
                    plugin.as_deref(),
                    &params_map,
                ) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Remove { packages, purge } => {
                if let Err(e) = commands::package::remove(&ctx, &packages, purge) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Rename { old, new } => {
                if let Err(e) = commands::package::rename(&ctx, &old, &new) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::AddDep {
                package,
                dependency,
            } => {
                if let Err(e) = commands::package::add_dep(&ctx, &package, &dependency) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::RemoveDep {
                package,
                dependency,
            } => {
                if let Err(e) = commands::package::remove_dep(&ctx, &package, &dependency) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::AddAlias { package, alias } => {
                if let Err(e) = commands::package::add_alias(&ctx, &package, &alias) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::RemoveAlias { package, alias } => {
                if let Err(e) = commands::package::remove_alias(&ctx, &package, &alias) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Enable { packages } => {
                if let Err(e) = commands::package::enable(&ctx, &packages) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Disable { packages } => {
                if let Err(e) = commands::package::disable(&ctx, &packages) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Info { package } => {
                if let Err(e) = commands::package::info(&ctx, &package) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Cat { package } => {
                if let Err(e) = commands::package::cat(&ctx, &package) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Cd { package } => {
                if let Err(e) = commands::package::cd(&ctx, package.as_deref()) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Install { packages, dry_run } => {
                if let Err(e) = commands::package::install(&ctx, &packages, dry_run) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Update { packages, dry_run } => {
                if let Err(e) = commands::package::update(&ctx, &packages, dry_run) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Uninstall {
                packages,
                all,
                dry_run,
            } => {
                if let Err(e) = commands::package::uninstall(&ctx, &packages, all, dry_run) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        },
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
    fn test_repo_option_defaults_to_default() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "init"]).unwrap();

        // Assert
        assert_eq!(cli.repo, "default");
    }

    #[test]
    fn test_repo_option_long() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--repo", "work", "init"]).unwrap();

        // Assert
        assert_eq!(cli.repo, "work");
    }

    #[test]
    fn test_repo_option_short() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "-r", "server", "init"]).unwrap();

        // Assert
        assert_eq!(cli.repo, "server");
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
    fn test_repo_add_help_shows_repo_argument() {
        // Arrange
        let cmd = Cli::command();
        let repo_cmd = cmd.find_subcommand("repo").unwrap();
        let add_cmd = repo_cmd.find_subcommand("add").unwrap();

        // Act
        let args: Vec<&str> = add_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "repo")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["repo"]);
    }

    #[test]
    fn test_repo_cd_help_shows_repo_argument() {
        // Arrange
        let cmd = Cli::command();
        let repo_cmd = cmd.find_subcommand("repo").unwrap();
        let cd_cmd = repo_cmd.find_subcommand("cd").unwrap();

        // Act
        let args: Vec<&str> = cd_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "repo")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["repo"]);
    }

    #[test]
    fn test_repo_remove_help_shows_repo_argument() {
        // Arrange
        let cmd = Cli::command();
        let repo_cmd = cmd.find_subcommand("repo").unwrap();
        let remove_cmd = repo_cmd.find_subcommand("remove").unwrap();

        // Act
        let args: Vec<&str> = remove_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "repo")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["repo"]);
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
}

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod config;
mod plan;
mod context;
mod state;
mod topo;

#[derive(Parser)]
#[command(name = "homeos", about = "Manage application install scripts and configurations across environments")]
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
        /// Remote URL to clone as the default repo
        url: Option<String>,
        /// Remove .git directory after cloning
        #[arg(long)]
        strip_git: bool,
    },
    /// Launch a shell in the default repository directory
    Cd,
    /// Install missing packages and update installed ones
    Apply,
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
}

#[derive(Subcommand)]
pub enum PluginCommands {
    /// List registered plugins
    List,
    /// List official plugins available from GitHub
    ListRemote,
}

#[derive(Subcommand)]
pub enum RepoCommands {
    /// List registered repositories
    List,
    /// Clone a remote repository
    Add {
        /// Repository name
        name: String,
        /// Remote URL to clone
        url: String,
    },
    /// Remove the local repository directory
    Remove {
        /// Repository name
        name: String,
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
        /// Dependencies for this package
        #[arg(long = "depends-on", num_args = 1..)]
        depends_on: Vec<String>,
    },
    /// Remove a package
    Remove {
        /// Package name
        package: String,
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
    /// Display all action scripts for a package
    Cat {
        /// Package name
        package: String,
    },
    /// Launch a shell in the package root or specific package directory
    Cd {
        /// Package name (optional — defaults to packages root)
        package: Option<String>,
    },
    /// Execute install action scripts
    Install {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Execute update action scripts
    Update {
        /// Package names
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Execute uninstall action scripts
    Uninstall {
        /// Package names
        #[arg(required_unless_present = "all")]
        packages: Vec<String>,
        /// Uninstall all installed packages (from state.yml)
        #[arg(long)]
        all: bool,
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
        Commands::Apply => {
            if let Err(e) = commands::package::apply(&ctx) {
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
        },
        Commands::Repo { command } => match command {
            RepoCommands::List => {
                if let Err(e) = commands::repo::list(&ctx) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            RepoCommands::Add { name, url } => {
                if let Err(e) = commands::repo::add(&ctx, &name, &url) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            RepoCommands::Remove { name } => {
                if let Err(e) = commands::repo::remove(&ctx, &name) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        },
        Commands::Package { command } => match command {
            PackageCommands::List => {
                if let Err(e) = commands::package::list(&ctx) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Add { package, depends_on } => {
                if let Err(e) = commands::package::add(&ctx, &package, &depends_on) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::AddDep { package, dependency } => {
                if let Err(e) = commands::package::add_dep(&ctx, &package, &dependency) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::RemoveDep { package, dependency } => {
                if let Err(e) = commands::package::remove_dep(&ctx, &package, &dependency) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Remove { package } => {
                if let Err(e) = commands::package::remove(&ctx, &package) {
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
            PackageCommands::Install { packages } => {
                if let Err(e) = commands::package::install(&ctx, &packages) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Update { packages } => {
                if let Err(e) = commands::package::update(&ctx, &packages) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Uninstall { packages, all } => {
                if let Err(e) = commands::package::uninstall(&ctx, &packages, all) {
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
}

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod config;
mod plan;
mod context;
mod state;

#[derive(Parser)]
#[command(name = "homeos", about = "Manage application install scripts and configurations across environments")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Override the base data directory (defaults to OS data directory)
    #[arg(long, global = true, hide = true)]
    pub base_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create the initial repository structure
    Init,
    /// Launch a shell in the default repository directory
    Cd,
    /// Manage packages
    Package {
        #[command(subcommand)]
        command: PackageCommands,
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
    let ctx = context::Context::new(cli.base_dir);

    match cli.command {
        Commands::Init => {
            if let Err(e) = commands::init::run(&ctx) {
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

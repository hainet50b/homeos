use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod config;
mod confirm;
mod context;

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
    },
    /// Remove a package
    Remove {
        /// Package name
        package: String,
    },
    /// Enable a package
    Enable {
        /// Package name
        package: String,
    },
    /// Disable a package
    Disable {
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
        #[arg(required = true)]
        packages: Vec<String>,
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
            PackageCommands::Add { package } => {
                if let Err(e) = commands::package::add(&ctx, &package) {
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
            PackageCommands::Enable { package } => {
                if let Err(e) = commands::package::enable(&ctx, &package) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            PackageCommands::Disable { package } => {
                if let Err(e) = commands::package::disable(&ctx, &package) {
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
            PackageCommands::Uninstall { packages } => println!("package uninstall: {packages:?}"),
        },
    }
}

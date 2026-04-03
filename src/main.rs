use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    /// Execute install action scripts
    Install {
        /// Package name (optional, operates on all if omitted)
        package: Option<String>,
    },
    /// Execute update action scripts
    Update {
        /// Package name (optional, operates on all if omitted)
        package: Option<String>,
    },
    /// Execute uninstall action scripts
    Uninstall {
        /// Package name (optional, operates on all if omitted)
        package: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let ctx = context::Context::new(cli.base_dir);

    match cli.command {
        Commands::Init => {
            println!("init: {:?}", ctx.repos_dir());
        }
        Commands::Cd => {
            println!("cd: {:?}", ctx.default_repo_dir());
        }
        Commands::Package { command } => match command {
            PackageCommands::List => println!("package list"),
            PackageCommands::Add { package } => println!("package add: {package}"),
            PackageCommands::Remove { package } => println!("package remove: {package}"),
            PackageCommands::Install { package } => println!("package install: {package:?}"),
            PackageCommands::Update { package } => println!("package update: {package:?}"),
            PackageCommands::Uninstall { package } => println!("package uninstall: {package:?}"),
        },
    }
}

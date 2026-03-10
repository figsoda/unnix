use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

/// Use Nix packages without installing Nix
#[derive(Parser)]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    #[command(flatten)]
    pub global: GlobalArgs,
}

#[derive(Subcommand)]
pub enum Command {
    /// Cache everything required for `unnix env` on the current system
    Cache,

    /// Set up environment variables for CI
    Ci(CiArgs),

    /// Enter the development environment
    Env(EnvArgs),

    /// Create a new unnix manifest
    Init(InitArgs),

    /// Generate a new lockfile or keep it up to date with the manifest
    Lock,

    /// Print information about the project
    Print(PrintArgs),

    /// Update the store paths in the lockfile
    Update,
}

#[derive(Parser)]
pub struct CiArgs {
    #[command(subcommand)]
    pub command: CiCommand,
}

#[derive(Subcommand)]
pub enum CiCommand {
    /// Set up environment variables for GitHub Actions
    Github,
}

#[derive(Parser)]
pub struct EnvArgs {
    /// Specify the command to run instead of $SHELL
    pub command: Option<Vec<String>>,
}

#[derive(Parser)]
pub struct InitArgs {
    /// Specify the list of packages
    #[arg(short, long, num_args = 0 ..)]
    pub packages: Vec<String>,

    /// Specify the list of supported systems
    #[arg(short, long, num_args = 1 ..)]
    pub systems: Vec<String>,
}

#[derive(Parser)]
pub struct PrintArgs {
    #[command(subcommand)]
    pub command: PrintCommand,
}

#[derive(Subcommand)]
pub enum PrintCommand {
    /// Print shell code for the development environment
    Env,
}

#[derive(Parser)]
pub struct GlobalArgs {
    /// Specify the directory the unnix manifest is in
    #[arg(short, long, global = true)]
    pub directory: Option<Utf8PathBuf>,

    /// Assert the lockfile is up to date
    #[arg(long, global = true)]
    pub locked: bool,
}

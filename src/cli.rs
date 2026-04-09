use std::str::FromStr;

use camino::Utf8PathBuf;
use clap::{
    Parser, Subcommand,
    builder::{Styles, styling::AnsiColor},
};
use miette::IntoDiagnostic;

/// Reproducible Nix environments without installing Nix
#[derive(Parser)]
#[command(styles = styles(), version)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,

    #[command(flatten)]
    pub global: GlobalArgs,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add packages to an unnix manifest
    Add(AddArgs),

    /// Cache everything required for `unnix env` on the current system
    Cache(CacheArgs),

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
pub struct AddArgs {
    /// The list of packages to add
    #[arg(name = "PACKAGE")]
    pub packages: Vec<String>,
}

#[derive(Parser)]
pub struct CacheArgs {
    #[command(flatten)]
    pub system: SystemArgs,
}

#[derive(Parser)]
pub struct CiArgs {
    #[command(subcommand)]
    pub command: CiCommand,

    #[command(flatten)]
    pub system: SystemArgs,
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

    #[command(flatten)]
    pub system: SystemArgs,
}

#[derive(Parser)]
pub struct InitArgs {
    /// Specify the list of packages
    #[arg(short, long, num_args = 0 ..)]
    pub packages: Vec<String>,

    /// Specify the list of supported systems
    #[arg(long, num_args = 1 ..)]
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
    Env(PrintEnvArgs),
}

#[derive(Parser)]
pub struct PrintEnvArgs {
    #[command(flatten)]
    pub system: SystemArgs,
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

#[derive(Parser)]
pub struct SystemArgs {
    /// Specify the host system of the packages
    #[arg(long, env = "UNNIX_SYSTEM", global = true)]
    pub system: Option<String>,
}

impl<T: FromStr> TryInto<Option<T>> for SystemArgs
where
    Result<Option<T>, T::Err>: IntoDiagnostic<Option<T>, T::Err>,
{
    type Error = miette::Error;

    fn try_into(self) -> Result<Option<T>, Self::Error> {
        self.system.map(|x| x.parse()).transpose().into_diagnostic()
    }
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Blue.on_default().bold())
        .placeholder(AnsiColor::Blue.on_default())
        .usage(AnsiColor::Green.on_default().bold())
}

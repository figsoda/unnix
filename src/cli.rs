use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Cache,
    Env(EnvArgs),
    Lock,
    Print(PrintArgs),
    Update,
}

#[derive(Parser)]
pub struct EnvArgs {
    pub command: Option<Vec<String>>,
}

#[derive(Parser)]
pub struct PrintArgs {
    #[command(subcommand)]
    pub command: PrintCommand,
}

#[derive(Subcommand)]
pub enum PrintCommand {
    Env,
}

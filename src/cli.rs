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
    Update,
}

#[derive(Parser)]
pub struct EnvArgs {
    pub command: Option<Vec<String>>,
}

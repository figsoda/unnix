use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Lock,
    Shell(ShellArgs),
    Update,
}

#[derive(Parser)]
pub struct ShellArgs {
    #[arg(short, long)]
    pub command: Option<Vec<String>>,
}

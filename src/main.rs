mod cli;
mod command;
mod lockfile;
mod manifest;
mod package;
mod source;
mod state;
mod store;
mod system;

use std::io::stderr;

use clap::Parser;
use miette::{IntoDiagnostic, Result};
use supports_color::Stream;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

use crate::{
    cli::{Args, Command},
    manifest::Manifest,
    state::State,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(supports_color::on(Stream::Stderr).is_some())
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("UNNIX_LOG")
                .from_env()
                .into_diagnostic()?,
        )
        .with_writer(stderr)
        .without_time()
        .init();

    let args = Args::parse();
    let manifest = Manifest::from_dir(".".into())?;
    let mut state = State::new(manifest)?;

    match args.command {
        Command::Lock => {
            command::lock(&mut state).await?;
        }
        Command::Shell(args) => {
            command::shell(&mut state, args).await?;
        }
        Command::Update => {
            command::update(&mut state).await?;
        }
    }

    Ok(())
}

// https://github.com/rust-lang/rust/issues/147648
#![allow(unused_assignments)]

mod cli;
mod command;
mod lockfile;
mod manifest;
mod package;
mod source;
mod state;
mod store;
mod system;

use clap::Parser;
use miette::{IntoDiagnostic, Result};
use supports_color::Stream;
use tracing::level_filters::LevelFilter;
use tracing_indicatif::{IndicatifLayer, style::ProgressStyle};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    cli::{Args, Command},
    manifest::Manifest,
    state::State,
};

#[tokio::main]
async fn main() -> Result<()> {
    let progress = IndicatifLayer::new()
        .with_max_progress_bars(1, None)
        .with_progress_style(
            ProgressStyle::with_template("{spinner:.blue} {bar:40} [{pos:.green}/{len:.yellow}]")
                .into_diagnostic()?,
        );

    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(supports_color::on(Stream::Stderr).is_some())
        .with_writer(progress.get_stderr_writer())
        .without_time()
        .with_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_env_var("UNNIX_LOG")
                .from_env()
                .into_diagnostic()?,
        );

    tracing_subscriber::registry()
        .with(layer)
        .with(progress)
        .init();

    let args = Args::parse();
    let manifest = Manifest::from_dir(".".into())?;
    let state = State::new(manifest)?;

    match args.command {
        Command::Cache => {
            command::cache(state).await?;
        }
        Command::Env(args) => {
            command::env(state, args).await?;
        }
        Command::Lock => {
            command::lock(state).await?;
        }
        Command::Print(args) => {
            command::print(state, args).await?;
        }
        Command::Update => {
            command::update(state).await?;
        }
    }

    Ok(())
}

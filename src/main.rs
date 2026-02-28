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

use crate::cli::{Args, Command};

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
    match args.command {
        Command::Cache => {
            command::cache(args.global).await?;
        }
        Command::Env(env_args) => {
            command::env(args.global, env_args).await?;
        }
        Command::Lock => {
            command::lock(args.global).await?;
        }
        Command::Print(print_args) => {
            command::print(args.global, print_args).await?;
        }
        Command::Update => {
            command::update(args.global).await?;
        }
    }

    Ok(())
}

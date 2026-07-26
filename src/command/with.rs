use std::{env::var_os, os::unix::process::CommandExt, process::Command};

use miette::{Report, Result, WrapErr};
use tracing::warn;

use crate::{
    cli::{GlobalArgs, WithArgs},
    state::State,
};

pub async fn with(global: GlobalArgs, args: WithArgs) -> Result<()> {
    let mut state = State::new_locked(global, args.system.try_into()?).await?;
    state.filter(args.packages, args.env).await?;

    let mut cmd = if cfg!(target_os = "linux") {
        let mut cmd = state.bwrap()?;
        if let Some(args) = args.command {
            cmd.args(args);
        } else if let Some(shell) = var_os("SHELL") {
            cmd.arg(shell);
        } else {
            cmd.arg("sh");
        }
        cmd
    } else {
        // TODO: hook into libSystem to redirect paths
        warn!(
            "`unnix with` is only properly supported on linux, please mount or symlink {} to /nix/store manually",
            state.store.path,
        );

        if let Some(args) = args.command {
            let mut args = args.into_iter();
            let mut cmd = Command::new(args.next().wrap_err("expected at least one argument")?);
            cmd.args(args);
            cmd
        } else if let Some(shell) = var_os("SHELL") {
            Command::new(shell)
        } else {
            Command::new("sh")
        }
    };

    cmd.envs(state.env().await?);
    Err(Report::from_err(cmd.exec()))
}

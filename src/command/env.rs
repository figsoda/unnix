use std::{env::var_os, os::unix::process::CommandExt};

use miette::{Report, Result};

use crate::{
    cli::{EnvArgs, GlobalArgs},
    state::State,
};

pub async fn env(global: GlobalArgs, args: EnvArgs) -> Result<()> {
    let mut state = State::new(global)?;
    state.lock().await?;

    let mut cmd = state.bwrap();
    cmd.envs(state.env().await?);

    if let Some(args) = args.command {
        cmd.args(args);
    } else if let Some(shell) = var_os("SHELL") {
        cmd.arg(shell);
    } else {
        cmd.arg("sh");
    }

    Err(Report::from_err(cmd.exec()))
}

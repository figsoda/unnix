use std::{env::var_os, ffi::OsString, os::unix::process::CommandExt};

use itertools::Itertools;
use miette::{Report, Result};

use crate::{cli::ShellArgs, state::State};

pub async fn shell(state: &mut State, args: ShellArgs) -> Result<()> {
    state.lock().await?;
    let paths: Vec<_> = state.lockfile.outputs(&state.system).collect();

    state.queue.extend(paths.iter().cloned());
    state.pull().await?;

    let mut path_var: OsString = paths
        .iter()
        .map(|path| format!("/nix/store/{path}/bin"))
        .join(":")
        .into();

    if let Some(paths) = var_os("PATH") {
        path_var.push(":");
        path_var.push(paths);
    }

    let mut cmd = state.bwrap();
    cmd.env("PATH", path_var);

    if let Some(args) = args.command {
        cmd.args(args);
    } else if let Some(shell) = var_os("SHELL") {
        cmd.arg(shell);
    } else {
        cmd.arg("sh");
    }

    Err(Report::from_err(cmd.exec()))
}

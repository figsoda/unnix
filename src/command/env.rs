use std::{env::var_os, ffi::OsString, os::unix::process::CommandExt};

use itertools::Itertools;
use miette::{Report, Result};

use crate::{cli::EnvArgs, state::State};

pub async fn env(state: &mut State, args: EnvArgs) -> Result<()> {
    state.lock().await?;
    let paths: Vec<_> = state.lockfile.outputs(&state.system).collect();

    let mut path_var: OsString = paths
        .iter()
        .flat_map(|path| state.canonicalize_subpath(path, "bin"))
        .join(":")
        .into();

    let mut pkg_config_path: OsString = paths
        .iter()
        .flat_map(|path| state.canonicalize_subpath(path, "lib/pkgconfig"))
        .join(":")
        .into();

    state.pull(paths).await?;

    if let Some(paths) = var_os("PATH") {
        path_var.push(":");
        path_var.push(paths);
    }

    if let Some(paths) = var_os("PKG_CONFIG_PATH") {
        pkg_config_path.push(":");
        pkg_config_path.push(paths);
    }

    let mut cmd = state.bwrap();
    cmd.env("PATH", path_var)
        .env("PKG_CONFIG_PATH", pkg_config_path);

    if let Some(args) = args.command {
        cmd.args(args);
    } else if let Some(shell) = var_os("SHELL") {
        cmd.arg(shell);
    } else {
        cmd.arg("sh");
    }

    Err(Report::from_err(cmd.exec()))
}

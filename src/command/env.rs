use std::{env::var_os, ffi::OsString, os::unix::process::CommandExt};

use itertools::Itertools;
use miette::{Report, Result};

use crate::{cli::EnvArgs, state::State};

pub async fn env(state: &mut State, args: EnvArgs) -> Result<()> {
    state.lock().await?;
    let mut paths: Vec<_> = state.lockfile.outputs(&state.system).collect();

    state.pull(paths.clone()).await?;
    paths.extend(state.store.propagated_build_inputs(paths.clone()).await?);

    let mut path_var: OsString = paths
        .iter()
        .flat_map(|path| state.store.canonicalize_subpath(path, "bin"))
        .join(":")
        .into();

    let mut nix_ldflags: OsString = paths
        .iter()
        .flat_map(|path| state.store.canonicalize_subpath(path, "lib"))
        .map(|path| format!("-L{path}"))
        .join(" ")
        .into();

    let mut pkg_config_path: OsString = paths
        .iter()
        .flat_map(|path| state.store.canonicalize_subpath(path, "lib/pkgconfig"))
        .join(":")
        .into();

    if let Some(paths) = var_os("PATH") {
        path_var.push(":");
        path_var.push(paths);
    }

    if let Some(paths) = var_os("NIX_LDFLAGS") {
        nix_ldflags.push(" ");
        nix_ldflags.push(paths);
    }

    if let Some(paths) = var_os("PKG_CONFIG_PATH") {
        pkg_config_path.push(":");
        pkg_config_path.push(paths);
    }

    let mut cmd = state.bwrap();
    cmd.env("PATH", path_var)
        .env("NIX_LDFLAGS", nix_ldflags)
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

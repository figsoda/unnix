use std::{env::var_os, ffi::OsString, os::unix::process::CommandExt};

use itertools::Itertools;
use miette::{IntoDiagnostic, Report, Result};
use strfmt::strfmt;

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

    let mut library_path: OsString = paths
        .iter()
        .flat_map(|path| state.store.canonicalize_subpath(path, "lib"))
        .join(":")
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

    if let Some(paths) = var_os("LIBRARY_PATH") {
        library_path.push(":");
        library_path.push(paths);
    }

    if let Some(paths) = var_os("PKG_CONFIG_PATH") {
        pkg_config_path.push(":");
        pkg_config_path.push(paths);
    }

    let mut cmd = state.bwrap();
    cmd.env("PATH", path_var)
        .env("LIBRARY_PATH", library_path)
        .env("PKG_CONFIG_PATH", pkg_config_path);

    let pkgs = state
        .lockfile
        .systems
        .get(&state.system)
        .iter()
        .flat_map(|pkgs| {
            pkgs.iter().flat_map(|(name, pkg)| {
                pkg.outputs.iter().map(move |(output, path)| {
                    (format!("{name}.{output}"), format!("/nix/store/{path}"))
                })
            })
        })
        .collect();
    for (name, value) in &state.manifest.env {
        cmd.env(name, strfmt(value, &pkgs).into_diagnostic()?);
    }

    if let Some(args) = args.command {
        cmd.args(args);
    } else if let Some(shell) = var_os("SHELL") {
        cmd.arg(shell);
    } else {
        cmd.arg("sh");
    }

    Err(Report::from_err(cmd.exec()))
}

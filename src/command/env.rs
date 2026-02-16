use std::{env::var_os, os::unix::process::CommandExt};

use miette::{IntoDiagnostic, Report, Result};
use strfmt::strfmt;

use crate::{cli::EnvArgs, state::State};

pub async fn env(state: &mut State, args: EnvArgs) -> Result<()> {
    state.lock().await?;
    let mut paths: Vec<_> = state.lockfile.outputs(&state.system).collect();

    state.pull(paths.clone()).await?;
    paths.extend(state.store.propagated_build_inputs(paths.clone()).await?);

    let path = state.store.prefix_env_subpaths("PATH", ":", &paths, "bin");

    let library_path = state
        .store
        .prefix_env_subpaths("LIBRARY_PATH", ":", &paths, "lib");

    let pkg_config_path =
        state
            .store
            .prefix_env_subpaths("PKG_CONFIG_PATH", ":", &paths, "lib/pkgconfig");

    let mut cmd = state.bwrap();
    cmd.env("PATH", path)
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

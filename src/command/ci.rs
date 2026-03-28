use std::env::var_os;

use miette::{IntoDiagnostic, Result, WrapErr, bail};
use tokio::{fs::File, io::AsyncWriteExt, join, try_join};
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::{
    cli::{CiArgs, CiCommand, GlobalArgs},
    state::State,
};

pub async fn ci(global: GlobalArgs, args: CiArgs) -> Result<()> {
    let state = State::new_locked(global, args.system.try_into()?).await?;
    match args.command {
        CiCommand::Github => github(state).await,
    }
}

async fn github(state: State) -> Result<()> {
    let Some(manifest) = state.manifest.systems.get(&state.system) else {
        bail!("system {} not supported by the manifest", state.system);
    };

    let mut paths = state.lockfile.collect_outputs(&state.system);
    state.pull(paths.clone()).await?;
    paths.extend(state.store.propagated_build_inputs(paths.clone()).await?);

    let write_env = async {
        let (mut github_env, env) = try_join!(
            async {
                let github_env = var_os("GITHUB_ENV").wrap_err("$GITHUB_ENV is unset")?;
                File::options()
                    .append(true)
                    .create(true)
                    .open(&github_env)
                    .await
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        format!(
                            "failed to open {} ($GITHUB_ENV)",
                            github_env.to_string_lossy(),
                        )
                    })
            },
            state.extra_env(&paths, manifest),
        )?;

        let uuid = Uuid::new_v4().simple().to_string();
        for (name, value) in env {
            let env = format!("{name}<<{uuid}\n{value}\n{uuid}\n");
            github_env
                .write_all(env.as_bytes())
                .await
                .into_diagnostic()?;
        }

        Result::<_>::Ok(())
    };

    let write_path = async {
        let (github_path, paths) = join!(
            async {
                let github_path = var_os("GITHUB_PATH").wrap_err("$GITHUB_PATH is unset")?;
                File::options()
                    .append(true)
                    .create(true)
                    .open(&github_path)
                    .await
                    .into_diagnostic()
                    .wrap_err_with(|| {
                        format!(
                            "failed to open {} ($GITHUB_PATH)",
                            github_path.to_string_lossy(),
                        )
                    })
            },
            state.store.subpaths(&paths, "bin").collect::<Vec<_>>(),
        );

        let mut github_path = github_path?;
        for path in paths {
            github_path
                .write_all(format!("{path}\n").as_bytes())
                .await
                .into_diagnostic()?;
        }

        Ok(())
    };

    try_join!(write_env, write_path)?;
    Ok(())
}

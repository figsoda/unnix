use miette::{IntoDiagnostic, Result};
use tokio::task::{JoinSet, LocalSet};

use crate::{cli::GlobalArgs, state::State};

pub async fn update(global: GlobalArgs) -> Result<()> {
    let mut state = State::new(global)?;
    let local = LocalSet::new();
    let mut tasks = JoinSet::new();

    for (system, manifest) in state.manifest.systems {
        let lockfile = state.lockfile.systems.entry(system).or_default();
        for (name, pkg) in manifest.packages {
            let lockfile = lockfile.clone();
            tasks.spawn_local_on(
                async move { lockfile.fetch(system, name, &pkg).await },
                &local,
            );
        }
    }

    local
        .run_until(async {
            while let Some(res) = tasks.join_next().await {
                res.into_diagnostic()??;
            }
            Result::<_>::Ok(())
        })
        .await?;

    state.lockfile.write_dir(&state.dir)
}

use miette::{IntoDiagnostic, Result};
use tokio::task::{JoinSet, LocalSet};

use crate::state::State;

pub async fn update(mut state: State) -> Result<()> {
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

use std::rc::Rc;

use miette::{IntoDiagnostic, Result};
use tokio::task::{JoinSet, LocalSet};
use tracing::{field::Empty, info_span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::{cli::GlobalArgs, lockfile::SystemLockfile, state::State};

pub async fn update(global: GlobalArgs) -> Result<()> {
    let mut state = State::new(global)?;
    let span = info_span!("update", indicatif.pb_show = Empty);
    span.pb_set_message("updating lockfile");
    span.pb_set_length(0);
    span.pb_start();

    let local = LocalSet::new();
    let mut tasks = JoinSet::new();

    for (system, manifest) in state.manifest.systems {
        let lockfile = Rc::new(SystemLockfile::default());
        for (name, pkg) in manifest.packages {
            let lockfile = lockfile.clone();
            let span = span.clone();
            tasks.spawn_local_on(
                async move {
                    span.pb_inc_length(1);
                    lockfile.fetch(system, name, &pkg).await?;
                    span.pb_inc(1);
                    Result::<_>::Ok(())
                },
                &local,
            );
        }
        state.lockfile.systems.insert(system, lockfile);
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

use miette::Result;

use crate::{cli::GlobalArgs, state::State};

pub async fn cache(global: GlobalArgs) -> Result<()> {
    let state = State::new_locked(global).await?;
    state
        .pull(state.lockfile.collect_outputs(&state.system))
        .await?;
    Ok(())
}

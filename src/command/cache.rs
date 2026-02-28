use miette::Result;

use crate::{cli::GlobalArgs, state::State};

pub async fn cache(global: GlobalArgs) -> Result<()> {
    let mut state = State::new(global)?;
    state.lock().await?;
    state
        .pull(state.lockfile.collect_outputs(&state.system))
        .await?;
    Ok(())
}

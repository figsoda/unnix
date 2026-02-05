use miette::Result;

use crate::state::State;

pub async fn cache(state: &mut State) -> Result<()> {
    state.lock().await?;
    state
        .pull(state.lockfile.outputs(&state.system).collect())
        .await?;
    Ok(())
}

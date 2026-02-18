use miette::Result;

use crate::state::State;

pub async fn cache(mut state: State) -> Result<()> {
    state.lock().await?;
    state
        .pull(state.lockfile.outputs(&state.system).collect())
        .await?;
    Ok(())
}

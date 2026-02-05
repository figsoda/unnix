use miette::Result;

use crate::state::State;

pub async fn cache(state: &mut State) -> Result<()> {
    state.lock().await?;
    state.queue.extend(state.lockfile.outputs(&state.system));
    state.pull().await?;
    Ok(())
}

use miette::Result;

use crate::state::State;

pub async fn lock(state: &mut State) -> Result<()> {
    state.lock().await
}

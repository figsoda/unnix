use miette::Result;

use crate::state::State;

pub async fn lock(mut state: State) -> Result<()> {
    state.lock().await
}

use miette::Result;

use crate::{cli::GlobalArgs, state::State};

pub async fn lock(global: GlobalArgs) -> Result<()> {
    State::new_locked(global, None).await?;
    Ok(())
}

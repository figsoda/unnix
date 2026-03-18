use miette::Result;

use crate::{
    cli::{CacheArgs, GlobalArgs},
    state::State,
};

pub async fn cache(global: GlobalArgs, args: CacheArgs) -> Result<()> {
    let state = State::new_locked(global, args.system.try_into()?).await?;
    state
        .pull(state.lockfile.collect_outputs(&state.system))
        .await?;
    Ok(())
}

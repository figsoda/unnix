use miette::Result;

use crate::state::State;

pub async fn update(state: &mut State) -> Result<()> {
    for (name, pkg) in &state.manifest.packages {
        state
            .lockfile
            .fetch(state.system, name.clone(), pkg)
            .await?;
    }
    state.lockfile.write_dir(&state.dir)?;
    Ok(())
}

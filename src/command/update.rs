use miette::Result;

use crate::state::State;

pub async fn update(mut state: State) -> Result<()> {
    for (system, manifest) in state.manifest.systems {
        for (name, pkg) in manifest.packages {
            state.lockfile.fetch(system, name.clone(), &pkg).await?;
        }
    }
    state.lockfile.write_dir(&state.dir)?;
    Ok(())
}

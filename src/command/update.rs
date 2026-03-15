use std::rc::Rc;

use miette::Result;
use tracing::{field::Empty, info_span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::{cli::GlobalArgs, lockfile::SystemLockfile, resolver::ResolverJobs, state::State};

pub async fn update(global: GlobalArgs) -> Result<()> {
    let mut state = State::new(global)?;

    let span = info_span!("update", indicatif.pb_show = Empty);
    span.pb_set_message("updating lockfile");
    span.pb_set_length(0);
    span.pb_start();
    let mut jobs = ResolverJobs::new(span);

    for (system, manifest) in state.manifest.systems {
        let lockfile = Rc::new(SystemLockfile::default());
        for (name, pkg) in manifest.packages {
            jobs.add(name.clone(), pkg.key()?, &pkg, system)?;
        }
        state.lockfile.systems.insert(system, lockfile);
    }

    jobs.resolve(&state.lockfile).await?;
    state.lockfile.write_dir(&state.dir)
}

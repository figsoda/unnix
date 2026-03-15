use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use dashmap::DashMap;
use miette::{IntoDiagnostic, Result, WrapErr, miette};
use reqwest::{Method, header::ACCEPT};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::Semaphore,
    task::{JoinSet, LocalSet},
};
use tracing::{Span, debug};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::{
    lockfile::{Lockfile, PackageLock},
    package::Base64Hash,
    resolver::format,
    state::HTTP_CLIENT,
    store::path::StorePath,
    system::System,
};

#[derive(Default)]
pub struct HydraJobs {
    jobs: BTreeMap<System, BTreeMap<Rc<str>, (Base64Hash, HydraPackage)>>,
}

#[derive(Debug, Serialize)]
pub struct HydraResolver {
    pub base: Rc<str>,
    pub project: Rc<str>,
    pub jobset: Rc<str>,
    pub job: String,
}

struct HydraPackage {
    base: Rc<str>,

    project: Rc<str>,

    jobset: Rc<str>,

    job: String,

    system: System,

    outputs: Rc<BTreeSet<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Build {
    Ok {
        buildoutputs: BTreeMap<String, Output>,
    },
    Err {
        error: String,
    },
}

#[derive(Deserialize)]
struct Output {
    path: String,
}

impl HydraJobs {
    pub async fn resolve(self, span: &Span, lockfile: &Lockfile) -> Result<()> {
        let locks = Rc::new(DashMap::new());
        let local = LocalSet::new();
        let mut tasks = JoinSet::new();

        for (system, jobs) in self.jobs {
            let lockfile = &lockfile.systems[&system];
            for (name, (key, job)) in jobs {
                let lockfile = lockfile.clone();
                let locks = locks.clone();
                let span = span.clone();
                tasks.spawn_local_on(
                    async move {
                        // allow at most 4 concurrent clients per hydra instance
                        let lock = locks
                            .entry(job.base.clone())
                            .or_insert_with(|| Rc::new(Semaphore::new(4)))
                            .value()
                            .clone();
                        let _permit = lock.acquire().await.into_diagnostic()?;

                        let outputs = job.resolve().await?;
                        lockfile.inner.insert(name, PackageLock { key, outputs });
                        span.pb_inc(1);

                        Result::<_>::Ok(())
                    },
                    &local,
                );
            }
        }

        local
            .run_until(async {
                while let Some(res) = tasks.join_next().await {
                    res.into_diagnostic()??;
                }
                Ok(())
            })
            .await
    }

    pub fn add(
        &mut self,
        hydra: &HydraResolver,
        name: Rc<str>,
        key: Base64Hash,
        attribute: &str,
        system: System,
        outputs: Rc<BTreeSet<String>>,
    ) -> Result<()> {
        let pkg = HydraPackage {
            base: hydra.base.clone(),
            project: hydra.project.clone(),
            jobset: hydra.jobset.clone(),
            job: format(&hydra.job, attribute, system)?,
            system,
            outputs,
        };

        self.jobs
            .entry(system)
            .or_default()
            .insert(name, (key, pkg));

        Ok(())
    }
}

impl HydraPackage {
    async fn resolve(&self) -> Result<BTreeMap<String, StorePath>> {
        let url = format!(
            "{}/job/{}/{}/{}/latest-for/{}",
            self.base, self.project, self.jobset, self.job, self.system,
        );

        debug!(url);

        let build: Build = HTTP_CLIENT
            .request(Method::GET, url)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .into_diagnostic()?
            .json()
            .await
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "failed to parse hydra response for {} on {}",
                    self.job,
                    self.system,
                )
            })?;

        match build {
            Build::Ok { buildoutputs } => {
                let mut outputs = buildoutputs
                    .into_iter()
                    .map(|(name, output)| Ok((name, StorePath::new(&output.path)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?;

                if !self.outputs.is_empty() {
                    outputs.retain(|name: &String, _| self.outputs.contains(name.as_str()));
                }

                Ok(outputs)
            }

            Build::Err { error } => {
                let e = miette!(error).wrap_err(format!(
                    "no successful build found for {} on {}",
                    self.job, self.system,
                ));
                Err(e)
            }
        }
    }
}

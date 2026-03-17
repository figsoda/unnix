use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

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
    jobs: BTreeMap<Rc<str>, BTreeMap<System, Vec<HydraPackage>>>,
}

#[derive(Debug, Serialize)]
pub struct HydraResolver {
    pub base: Rc<str>,
    pub project: Rc<str>,
    pub jobset: Rc<str>,
    pub job: String,
}

struct HydraPackage {
    name: Rc<str>,
    key: Base64Hash,
    project: Rc<str>,
    jobset: Rc<str>,
    job: String,
    outputs: Rc<BTreeSet<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Build {
    Ok {
        buildoutputs: BTreeMap<Rc<str>, Output>,
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
        let local = LocalSet::new();
        let mut tasks = JoinSet::new();

        for (base, jobs) in self.jobs {
            // allow at most 4 concurrent clients per hydra instance
            let semaphore = Rc::new(Semaphore::new(4));

            for (system, pkgs) in jobs {
                let lockfile = &lockfile.systems[&system];
                for pkg in pkgs {
                    let base = base.clone();
                    let lockfile = lockfile.clone();
                    let semaphore = semaphore.clone();
                    let span = span.clone();
                    tasks.spawn_local_on(
                        async move {
                            let _permit = semaphore.acquire().await.into_diagnostic()?;
                            let outputs = pkg.resolve(&base, system).await?;
                            lockfile.inner.insert(
                                pkg.name,
                                PackageLock {
                                    key: pkg.key,
                                    outputs,
                                },
                            );
                            span.pb_inc(1);
                            Result::<_>::Ok(())
                        },
                        &local,
                    );
                }
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
        package: &str,
        system: System,
        outputs: Rc<BTreeSet<String>>,
    ) -> Result<()> {
        let pkg = HydraPackage {
            name,
            key,
            project: hydra.project.clone(),
            jobset: hydra.jobset.clone(),
            job: format(&hydra.job, package, system)?,
            outputs,
        };

        self.jobs
            .entry(hydra.base.clone())
            .or_default()
            .entry(system)
            .or_default()
            .push(pkg);

        Ok(())
    }
}

impl HydraPackage {
    async fn resolve(&self, base: &str, system: System) -> Result<BTreeMap<Rc<str>, StorePath>> {
        let url = format!(
            "{base}/job/{}/{}/{}/latest-for/{system}",
            self.project, self.jobset, self.job,
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
                    "failed to parse hydra response for {} on {system}",
                    self.job,
                )
            })?;

        match build {
            Build::Ok { buildoutputs } => {
                let mut outputs = buildoutputs
                    .into_iter()
                    .map(|(name, output)| Ok((name, StorePath::new(&output.path)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?;

                if !self.outputs.is_empty() {
                    outputs.retain(|name, _| self.outputs.contains(name.as_ref()));
                }

                Ok(outputs)
            }

            Build::Err { error } => {
                let e = miette!(error).wrap_err(format!(
                    "no successful build found for {} on {system}",
                    self.job,
                ));
                Err(e)
            }
        }
    }
}

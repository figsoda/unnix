use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use miette::{IntoDiagnostic, Result, WrapErr, miette};
use serde::{Deserialize, Serialize};
use tokio::task::{JoinSet, LocalSet};
use tracing::{Span, debug};
use tracing_indicatif::span_ext::IndicatifSpanExt;
use url::Url;

use crate::{
    lockfile::{Lockfile, PackageLock},
    package::Base64Hash,
    resolver::format,
    state::HTTP_CLIENT,
    store::path::StorePath,
    system::System,
};

#[derive(Default)]
pub struct DevboxJobs {
    jobs: BTreeMap<DevboxPackage, BTreeMap<System, Vec<DevboxPackageLock>>>,
}

#[derive(Debug, Serialize)]
pub struct DevboxResolver {
    pub package: String,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct DevboxPackage {
    name: String,
    version: String,
}

struct DevboxPackageLock {
    name: Rc<str>,
    key: Base64Hash,
    outputs: Rc<BTreeSet<String>>,
}

#[derive(Deserialize)]
struct Resolved {
    systems: BTreeMap<String, SystemResolved>,
}

#[derive(Deserialize)]
struct SystemResolved {
    outputs: Vec<Output>,
}

#[derive(Deserialize)]
struct Output {
    name: String,
    path: String,
}

impl DevboxJobs {
    pub async fn resolve(self, span: &Span, lockfile: &Lockfile) -> Result<()> {
        let local = LocalSet::new();
        let mut tasks = JoinSet::new();

        for (pkg, systems) in self.jobs {
            let span = span.clone();
            tasks.spawn_local_on(
                async move {
                    let resolved = pkg.resolve(systems.keys().copied().collect()).await?;
                    span.pb_inc(1);
                    Result::<_>::Ok((systems, resolved))
                },
                &local,
            );
        }

        local
            .run_until(async {
                while let Some(res) = tasks.join_next().await {
                    let (systems, resolved) = res.into_diagnostic()??;
                    for (system, locks) in systems {
                        let lockfile = &lockfile.systems[&system];
                        let outputs = &resolved[&system];
                        for lock in locks {
                            let outputs = outputs
                                .iter()
                                .filter(|&(output, _)| {
                                    lock.outputs.is_empty() || lock.outputs.contains(output)
                                })
                                .map(|(output, path)| (output.clone(), path.clone()))
                                .collect();

                            lockfile.inner.insert(
                                lock.name,
                                PackageLock {
                                    key: lock.key,
                                    outputs,
                                },
                            );
                        }
                    }
                }
                Ok(())
            })
            .await
    }

    pub fn add(
        &mut self,
        devbox: &DevboxResolver,
        name: Rc<str>,
        key: Base64Hash,
        package: &str,
        system: System,
        outputs: Rc<BTreeSet<String>>,
    ) -> Result<()> {
        let pkg = format(&devbox.package, package, system)?;
        let pkg = if let Some((name, version)) = package.rsplit_once('@')
            && !version.is_empty()
        {
            DevboxPackage {
                name: name.into(),
                version: version.into(),
            }
        } else {
            DevboxPackage {
                name: pkg,
                version: "latest".into(),
            }
        };

        self.jobs
            .entry(pkg)
            .or_default()
            .entry(system)
            .or_default()
            .push(DevboxPackageLock { name, key, outputs });

        Ok(())
    }
}

impl DevboxPackage {
    async fn resolve(
        &self,
        systems: BTreeSet<System>,
    ) -> Result<BTreeMap<System, BTreeMap<String, StorePath>>> {
        let url = Url::parse_with_params(
            "https://search.devbox.sh/v2/resolve",
            [("name", &self.name), ("version", &self.version)],
        )
        .into_diagnostic()?;
        debug!("{}", url.as_str());

        let mut resolved: Resolved = HTTP_CLIENT
            .get(url)
            .send()
            .await
            .into_diagnostic()?
            .json()
            .await
            .into_diagnostic()
            .wrap_err_with(|| {
                miette!(
                    "failed to parse devbox response for {}@{}",
                    self.name,
                    self.version,
                )
            })?;

        systems
            .into_iter()
            .map(|system| {
                let outputs = resolved
                    .systems
                    .remove(&system.to_string())
                    .wrap_err_with(|| {
                        miette!(
                            "devbox package {}@{} does not support {system}",
                            self.name,
                            self.version,
                        )
                    })?
                    .outputs
                    .into_iter()
                    .map(|output| Ok((output.name, StorePath::new(&output.path)?)))
                    .collect::<Result<_>>()?;
                Ok((system, outputs))
            })
            .collect()
    }
}

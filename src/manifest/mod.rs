mod imp;

use std::{collections::HashMap, fs::read_to_string, rc::Rc, sync::Arc};

use camino::Utf8Path;
use miette::{IntoDiagnostic, Result, miette};
use url::Url;

use crate::{
    package::Package,
    source::{Source, hydra::Jobset},
};

#[derive(Debug)]
pub struct Manifest {
    pub packages: HashMap<String, Package>,
    pub caches: Vec<Arc<Url>>,
    pub env: HashMap<String, String>,
}

impl Manifest {
    pub fn from_dir(path: &Utf8Path) -> Result<Self> {
        let path = path.join("unnix.kdl");
        let text = read_to_string(&path).into_diagnostic()?;
        let manifest: imp::Manifest = knus::parse(&path, &text)?;

        let mut sources: HashMap<_, _> = manifest
            .sources
            .into_iter()
            .map(|source| match source {
                imp::Source::Hydra(hydra) => (
                    hydra.name,
                    Rc::new(Source::Hydra(Jobset {
                        base: hydra.base.inner,
                        project: hydra.project.inner,
                        jobset: hydra.jobset.inner,
                        job: hydra.job.inner,
                    })),
                ),
            })
            .collect();
        sources.entry("default".into()).or_default();

        let packages = manifest
            .packages
            .inner
            .into_iter()
            .map(|pkg| {
                let name = pkg.name;
                let pkg = Package {
                    attribute: pkg.attribute.unwrap_or_else(|| name.clone()),
                    outputs: pkg.outputs,
                    source: sources
                        .get(&pkg.source)
                        .ok_or_else(|| miette!("source {:?} not found", pkg.source))?
                        .clone(),
                };
                Ok((name, pkg))
            })
            .collect::<Result<_>>()?;

        let mut caches = Vec::new();
        if manifest.caches.default {
            caches.push(Arc::new(
                Url::parse("https://cache.nixos.org").into_diagnostic()?,
            ));
        }
        caches.extend(
            manifest
                .caches
                .inner
                .into_iter()
                .map(|cache| Arc::new(cache.url)),
        );

        let env = manifest
            .env
            .inner
            .into_iter()
            .map(|var| (var.name, var.value))
            .collect();

        Ok(Self {
            packages,
            caches,
            env,
        })
    }
}

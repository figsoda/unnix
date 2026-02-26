mod imp;
mod tests;

use std::{collections::BTreeMap, fs::read_to_string, rc::Rc, sync::Arc};

use camino::Utf8Path;
use miette::{IntoDiagnostic, Result, miette};
use url::Url;

use crate::{
    package::Package,
    source::{Source, hydra::Jobset},
    system::System,
};

#[derive(Debug)]
pub struct Manifest {
    pub systems: BTreeMap<System, SystemManifest>,
}

#[derive(Clone, Debug)]
pub struct SystemManifest {
    pub packages: BTreeMap<Rc<str>, Rc<Package>>,
    pub caches: Vec<Arc<Url>>,
    pub env: BTreeMap<Rc<str>, Rc<str>>,
}

impl Manifest {
    pub fn from_dir(path: &Utf8Path) -> Result<Self> {
        let path = path.join("unnix.kdl");
        let text = read_to_string(&path).into_diagnostic()?;
        Self::parse(path, &text)
    }

    fn parse(path: impl AsRef<str>, text: &str) -> Result<Self> {
        let manifest: imp::Manifest = knus::parse(path, text)?;

        let mut sources: BTreeMap<_, _> =
            manifest.sources.into_iter().map(transform_source).collect();
        sources.entry("default".into()).or_default();

        let packages = manifest
            .packages
            .inner
            .into_iter()
            .map(|pkg| transform_package(&sources, pkg))
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
            .map(|var| (var.name.into(), var.value.into()))
            .collect();

        let default = SystemManifest {
            packages,
            caches,
            env,
        };

        let mut systems: BTreeMap<_, _> = manifest
            .systems
            .inner
            .into_iter()
            .map(|system| (system.inner, default.clone()))
            .collect();

        for imp_manifest in manifest.system {
            for (system, manifest) in systems.iter_mut() {
                if let Some(arch) = imp_manifest.system.arch
                    && arch != system.arch
                {
                    continue;
                }

                if let Some(kernel) = imp_manifest.system.kernel
                    && kernel != system.kernel
                {
                    continue;
                }

                let mut sources = sources.clone();
                sources.extend(imp_manifest.sources.iter().cloned().map(transform_source));

                for pkg in &imp_manifest.packages.inner {
                    let (name, pkg) = transform_package(&sources, pkg.clone())?;
                    manifest.packages.insert(name, pkg);
                }

                manifest.caches.extend(
                    imp_manifest
                        .caches
                        .inner
                        .iter()
                        .cloned()
                        .map(|cache| Arc::new(cache.url)),
                );

                manifest.env.extend(
                    imp_manifest
                        .env
                        .inner
                        .iter()
                        .cloned()
                        .map(|var| (var.name.into(), var.value.into())),
                );
            }
        }

        Ok(Self { systems })
    }
}

fn transform_source(source: imp::Source) -> (String, Rc<Source>) {
    match source {
        imp::Source::Hydra(hydra) => (
            hydra.name,
            Rc::new(Source::Hydra(Jobset {
                base: hydra.base,
                project: hydra.project,
                jobset: hydra.jobset,
                job: hydra.job,
            })),
        ),
    }
}

fn transform_package(
    sources: &BTreeMap<String, Rc<Source>>,
    pkg: imp::Package,
) -> Result<(Rc<str>, Rc<Package>)> {
    let name: Rc<str> = pkg.name.into();
    let pkg = Rc::new(Package {
        attribute: pkg.attribute.map_or_else(|| name.clone(), Rc::from),
        outputs: pkg.outputs,
        source: sources
            .get(&pkg.source)
            .ok_or_else(|| miette!("source {:?} not found", pkg.source))?
            .clone(),
    });
    Ok((name, pkg))
}

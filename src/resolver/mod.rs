pub mod devbox;
pub mod hydra;

use std::{collections::HashMap, rc::Rc};

use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use strfmt::strfmt;
use tokio::try_join;
use tracing::Span;
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::{
    lockfile::Lockfile,
    package::{Base64Hash, Package},
    resolver::{
        devbox::{DevboxJobs, DevboxResolver},
        hydra::{HydraJobs, HydraResolver},
    },
    system::System,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum Resolver {
    Devbox(DevboxResolver),
    Hydra(HydraResolver),
}

pub struct ResolverJobs {
    span: Span,
    devbox: DevboxJobs,
    hydra: HydraJobs,
}

impl ResolverJobs {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            devbox: DevboxJobs::default(),
            hydra: HydraJobs::default(),
        }
    }

    pub async fn resolve(self, lockfile: &Lockfile) -> Result<()> {
        try_join!(
            self.devbox.resolve(&self.span, lockfile),
            self.hydra.resolve(&self.span, lockfile),
        )?;
        Ok(())
    }

    pub fn add(
        &mut self,
        name: Rc<str>,
        key: Base64Hash,
        pkg: &Package,
        system: System,
    ) -> Result<()> {
        self.span.pb_inc_length(1);

        match pkg.resolver.as_ref() {
            Resolver::Devbox(devbox) => {
                self.devbox
                    .add(devbox, name, key, &pkg.package, system, pkg.outputs.clone())?;
            }

            Resolver::Hydra(hydra) => {
                self.hydra
                    .add(hydra, name, key, &pkg.package, system, pkg.outputs.clone())?;
            }
        }

        Ok(())
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::Hydra(HydraResolver {
            base: "https://hydra.nixos.org".into(),
            project: "nixpkgs".into(),
            jobset: "unstable".into(),
            job: "{package}.{system}".into(),
        })
    }
}

fn format(template: &str, package: &str, system: System) -> Result<String> {
    let system = system.to_string();
    let mut params = HashMap::<String, _>::new();
    params.insert("package".into(), package);
    params.insert("system".into(), &system);
    strfmt(template, &params).into_diagnostic()
}

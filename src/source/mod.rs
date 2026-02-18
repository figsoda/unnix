pub mod hydra;

use std::collections::{BTreeMap, HashMap};

use enum_dispatch::enum_dispatch;
use miette::{IntoDiagnostic, Result};
use serde::Serialize;
use strfmt::strfmt;

use crate::{source::hydra::Jobset, store::path::StorePath, system::System};

#[enum_dispatch]
pub trait GetOutputs {
    async fn get_outputs(
        &self,
        attribute: &str,
        system: System,
    ) -> Result<BTreeMap<String, StorePath>>;
}

#[derive(Debug, Serialize)]
#[enum_dispatch(GetOutputs)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum Source {
    Hydra(Jobset),
}

impl Default for Source {
    fn default() -> Self {
        Self::Hydra(Jobset {
            base: "https://hydra.nixos.org".into(),
            project: "nixpkgs".into(),
            jobset: "unstable".into(),
            job: "{attribute}.{system}".into(),
        })
    }
}

fn format(template: &str, attribute: &str, system: System) -> Result<String> {
    let system = system.to_string();
    let mut params = HashMap::<String, _>::new();
    params.insert("attribute".into(), attribute);
    params.insert("system".into(), &system);
    strfmt(template, &params).into_diagnostic()
}

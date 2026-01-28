pub mod hydra;

use std::{collections::HashMap, rc::Rc};

use enum_dispatch::enum_dispatch;
use miette::Result;
use serde::{Deserialize, Serialize};

use crate::{source::hydra::Jobset, store::path::StorePath, system::System};

#[enum_dispatch]
pub trait GetOutputs {
    async fn get_outputs(
        &self,
        attribute: &str,
        system: System,
    ) -> Result<HashMap<String, Rc<StorePath>>>;
}

#[derive(Debug, Deserialize, Serialize)]
#[enum_dispatch(GetOutputs)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", tag = "type")]
pub enum Source {
    Hydra(Jobset),
}

impl Default for Source {
    fn default() -> Self {
        Self::Hydra(Jobset {
            domain: "hydra.nixos.org".into(),
            project: "nixpkgs".into(),
            jobset: "unstable".into(),
        })
    }
}

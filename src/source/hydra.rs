use std::{collections::HashMap, rc::Rc};

use miette::{IntoDiagnostic, Result};
use reqwest::{Client, Method, header::ACCEPT};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{source::GetOutputs, store::path::StorePath, system::System};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Jobset {
    pub domain: String,
    pub project: String,
    pub jobset: String,
}

#[derive(Deserialize)]
struct Build {
    buildoutputs: HashMap<String, Output>,
}

#[derive(Deserialize)]
struct Output {
    path: String,
}

impl GetOutputs for Jobset {
    async fn get_outputs(
        &self,
        attribute: &str,
        system: System,
    ) -> Result<HashMap<String, Rc<StorePath>>> {
        let url = format!(
            "https://{}/job/{}/{}/{}.{}/latest",
            self.domain, self.project, self.jobset, attribute, system,
        );

        debug!(url);

        let build: Build = Client::new()
            .request(Method::GET, url)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .into_diagnostic()?
            .json()
            .await
            .into_diagnostic()?;

        build
            .buildoutputs
            .into_iter()
            .map(|(name, output)| Ok((name, Rc::new(StorePath::new(&output.path)?))))
            .collect()
    }
}

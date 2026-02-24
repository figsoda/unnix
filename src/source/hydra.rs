use std::collections::BTreeMap;

use miette::{IntoDiagnostic, Result};
use reqwest::{Method, header::ACCEPT};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    source::{GetOutputs, format},
    state::HTTP_CLIENT,
    store::path::StorePath,
    system::System,
};

#[derive(Debug, Serialize)]
pub struct Jobset {
    pub base: String,
    pub project: String,
    pub jobset: String,
    pub job: String,
}

#[derive(Deserialize)]
struct Build {
    buildoutputs: BTreeMap<String, Output>,
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
    ) -> Result<BTreeMap<String, StorePath>> {
        let url = format!(
            "{}/job/{}/{}/{}/latest-for/{system}",
            self.base,
            self.project,
            self.jobset,
            format(&self.job, attribute, system)?,
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
            .into_diagnostic()?;

        build
            .buildoutputs
            .into_iter()
            .map(|(name, output)| Ok((name, StorePath::new(&output.path)?)))
            .collect()
    }
}

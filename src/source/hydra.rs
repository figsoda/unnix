use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};

use dashmap::DashMap;
use miette::{IntoDiagnostic, Result, WrapErr, miette};
use reqwest::{Method, header::ACCEPT};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
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

impl GetOutputs for Jobset {
    async fn get_outputs(
        &self,
        attribute: &str,
        system: System,
    ) -> Result<BTreeMap<String, StorePath>> {
        let job = format(&self.job, attribute, system)?;
        let url = format!(
            "{}/job/{}/{}/{job}/latest-for/{system}",
            self.base, self.project, self.jobset,
        );

        debug!(url);

        // allow at most 4 concurrent clients per hydra instance
        static LOCKS: LazyLock<DashMap<String, Arc<Semaphore>>> = LazyLock::new(DashMap::new);
        let lock = LOCKS
            .entry(self.base.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(4)))
            .value()
            .clone();
        let permit = lock.acquire().await.into_diagnostic()?;

        let build: Build = HTTP_CLIENT
            .request(Method::GET, url)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .into_diagnostic()?
            .json()
            .await
            .into_diagnostic()
            .wrap_err_with(|| miette!("failed to parse hydra response for {job} on {system}"))?;

        drop(permit);

        match build {
            Build::Ok { buildoutputs } => buildoutputs
                .into_iter()
                .map(|(name, output)| Ok((name, StorePath::new(&output.path)?)))
                .collect(),

            Build::Err { error } => {
                Err(miette!(error)
                    .wrap_err(format!("no successful build found for {job} on {system}")))
            }
        }
    }
}

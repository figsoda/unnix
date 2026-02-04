use std::{collections::BTreeSet, io::Cursor, process::Command, rc::Rc};

use camino::{Utf8Path, Utf8PathBuf};
use miette::{IntoDiagnostic, Result, bail};
use reqwest::{Client, StatusCode};
use tracing::{debug, info, warn};
use url::Url;

use crate::{
    lockfile::Lockfile,
    manifest::Manifest,
    store::{Store, nar::Narinfo, path::StorePath},
    system::System,
};

pub struct State {
    pub dir: Utf8PathBuf,
    pub lockfile: Lockfile,
    pub manifest: Manifest,
    pub queue: BTreeSet<Rc<StorePath>>,
    pub system: System,
    client: Client,
    downloaded: BTreeSet<Rc<StorePath>>,
    store: Store,
}

impl State {
    pub fn new(manifest: Manifest) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            dir: Utf8PathBuf::from("."),
            downloaded: BTreeSet::new(),
            lockfile: Lockfile::default(),
            manifest,
            queue: BTreeSet::new(),
            store: Store::new()?,
            system: System::host()?,
        })
    }

    pub async fn lock(&mut self) -> Result<()> {
        let old = Lockfile::from_dir(&self.dir)?;
        for (name, pkg) in &self.manifest.packages {
            self.lockfile
                .add(&old, self.system, name.clone(), pkg)
                .await?;
        }
        self.lockfile.write_dir(&self.dir)?;
        Ok(())
    }

    pub fn collect_outputs(&mut self) -> Vec<Rc<StorePath>> {
        let Some(packages) = self.lockfile.systems.get(&self.system) else {
            return Vec::new();
        };
        packages
            .values()
            .flat_map(|pkg| pkg.outputs.values())
            .cloned()
            .collect()
    }

    pub async fn pull(&mut self) -> Result<()> {
        while let Some(path) = self.queue.pop_first() {
            if self.downloaded.contains(&path) {
                continue;
            }

            let (cache, narinfo) = self.query(&path).await?;

            if !self.store.contains(&path) {
                info!("downloading {path} from {cache}");

                let nar = self
                    .client
                    .get(cache.join(&narinfo.url).into_diagnostic()?)
                    .send()
                    .await
                    .into_diagnostic()?
                    .bytes()
                    .await
                    .into_diagnostic()?;

                self.store
                    .unpack_nar(&path, Cursor::new(nar), narinfo.compression)
                    .await?;
            }

            self.downloaded.insert(path);
            for path in narinfo.references {
                self.queue.insert(Rc::new(path));
            }
        }
        Ok(())
    }

    pub fn bwrap(&self) -> Command {
        let mut cmd = Command::new("bwrap");

        cmd.arg("--bind").arg("/").arg("/");
        cmd.arg("--dev-bind").arg("/dev").arg("/dev");

        if Utf8Path::new("/nix/store").is_dir() {
            cmd.arg("--overlay-src").arg("/nix/store");
            cmd.arg("--overlay-src").arg(&self.store.path);
            cmd.arg("--ro-overlay").arg("/nix/store");
        } else {
            cmd.arg("--ro-bind").arg(&self.store.path).arg("/nix/store");
        }

        cmd.arg("--");
        cmd
    }

    async fn query(&self, path: &StorePath) -> Result<(&Url, Narinfo)> {
        for cache in &self.manifest.caches {
            debug!("checking {path} on {cache}");
            match self.query_one(path.hash(), cache).await {
                Ok(Some(narinfo)) => {
                    return Ok((cache, Narinfo::parse(&narinfo)?));
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("{e}");
                }
            }
        }

        bail!("{path} could not be found in any cache");
    }

    async fn query_one(&self, hash: &str, cache: &Url) -> Result<Option<String>> {
        let res = self
            .client
            .get(cache.join(&format!("{hash}.narinfo")).into_diagnostic()?)
            .send()
            .await
            .into_diagnostic()?;

        if res.status() == StatusCode::NOT_FOUND {
            Ok(None)
        } else {
            Ok(Some(
                res.error_for_status()
                    .into_diagnostic()?
                    .text()
                    .await
                    .into_diagnostic()?,
            ))
        }
    }
}

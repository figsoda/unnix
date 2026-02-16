use std::{collections::BTreeSet, io::Cursor, process::Command, sync::Arc};

use camino::{Utf8Path, Utf8PathBuf};
use miette::{IntoDiagnostic, Result, bail, miette};
use reqwest::{Client, StatusCode};
use tokio::{select, sync::mpsc, task::JoinSet, try_join};
use tracing::{debug, info, info_span, warn};
use tracing_indicatif::span_ext::IndicatifSpanExt;
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
    pub store: Arc<Store>,
    pub system: System,
}

impl State {
    pub fn new(manifest: Manifest) -> Result<Self> {
        Ok(Self {
            dir: Utf8PathBuf::from("."),
            lockfile: Lockfile::default(),
            manifest,
            store: Arc::new(Store::new()?),
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

    pub async fn pull(&mut self, paths: Vec<StorePath>) -> Result<()> {
        let span = info_span!("progress");
        span.pb_set_length(0);
        let _guard = span.enter();

        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(paths).map_err(|_| miette!("channel closed"))?;

        let client = Client::new();
        let mut downloaded = BTreeSet::new();
        let mut tasks = JoinSet::new();

        loop {
            let join_all = async {
                while let Some(res) = tasks.join_next().await {
                    res.into_diagnostic()??;
                }
                Result::<_>::Ok(())
            };

            let paths = select! {
                paths = rx.recv() => paths,
                res = join_all => {
                    res?;
                    if let Ok(paths) = rx.try_recv() {
                        Some(paths)
                    } else {
                        break;
                    }
                },
            };

            let Some(paths) = paths else {
                break;
            };

            for path in paths {
                if !downloaded.insert(path.clone()) {
                    continue;
                }

                let caches = self.manifest.caches.clone();
                let client = client.clone();
                let span = span.clone();
                let store = self.store.clone();
                let tx = tx.clone();

                tasks.spawn(async move {
                    if store.path.join(&path).symlink_metadata().is_ok()
                        && let Some(references) = store.get_references(path.hash()).await?
                    {
                        tx.send(references).map_err(|_| miette!("channel closed"))?;
                        return Ok(());
                    }

                    span.pb_inc_length(1);

                    let (cache, narinfo) = query(&client, &path, caches).await?;

                    info!("downloading {path} from {cache}");
                    let nar = cache.join(&narinfo.url).into_diagnostic()?;
                    tx.send(narinfo.references.clone())
                        .map_err(|_| miette!("channel closed"))?;

                    let put_references = store.put_references(path.hash(), &narinfo.references);
                    let unpack_nar = async {
                        let nar = client
                            .get(nar)
                            .send()
                            .await
                            .into_diagnostic()?
                            .bytes()
                            .await
                            .into_diagnostic()?;

                        store
                            .unpack_nar(&path, Cursor::new(nar), narinfo.compression)
                            .await
                    };

                    try_join!(put_references, unpack_nar)?;
                    span.pb_inc(1);
                    Result::<_>::Ok(())
                });
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
}

async fn query(
    client: &Client,
    path: &StorePath,
    caches: Vec<Arc<Url>>,
) -> Result<(Arc<Url>, Narinfo)> {
    for cache in caches {
        debug!("checking {path} on {cache}");
        match query_one(client, path.hash(), &cache).await {
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

async fn query_one(client: &Client, hash: &str, cache: &Url) -> Result<Option<String>> {
    let res = client
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

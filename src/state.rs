use std::{
    collections::{BTreeSet, HashMap},
    io::Cursor,
    process::Command,
    sync::{Arc, LazyLock},
};

use camino::{Utf8Path, Utf8PathBuf};
use miette::{IntoDiagnostic, Result, bail, miette};
use reqwest::{Client, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{Jitter, RetryTransientMiddleware, policies::ExponentialBackoff};
use strfmt::strfmt;
use tokio::{
    select,
    sync::mpsc,
    task::{JoinSet, LocalSet},
    try_join,
};
use tracing::{debug, field::Empty, info, info_span, warn};
use tracing_indicatif::{span_ext::IndicatifSpanExt, style::ProgressStyle};
use url::Url;

use crate::{
    cli::GlobalArgs,
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

pub static HTTP_CLIENT: LazyLock<ClientWithMiddleware> = LazyLock::new(|| {
    let client = Client::builder()
        .user_agent(concat!("unnix/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap();

    let policy = ExponentialBackoff::builder()
        .jitter(Jitter::Bounded)
        .build_with_max_retries(4);

    ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(policy))
        .build()
});

impl State {
    pub fn new(global: GlobalArgs) -> Result<Self> {
        let dir = global.directory.unwrap_or_else(|| ".".into());
        let manifest = Manifest::from_dir(&dir)?;

        Ok(Self {
            dir,
            lockfile: Lockfile::default(),
            manifest,
            store: Arc::new(Store::new()?),
            system: System::host()?,
        })
    }

    pub async fn lock(&mut self) -> Result<()> {
        let span = info_span!("lock", indicatif.pb_show = Empty);
        span.pb_set_message("generating lockfile");
        span.pb_set_length(0);
        span.pb_start();

        let old = Lockfile::from_dir(&self.dir)?;
        let local = LocalSet::new();
        let mut tasks = JoinSet::new();

        for (&system, manifest) in &self.manifest.systems {
            let lockfile = self.lockfile.systems.entry(system).or_default().clone();
            for (name, pkg) in &manifest.packages {
                if let Some(old) = old.systems.get(&system)
                    && let Some(old) = old.inner.get(name)
                    && old.key == pkg.key()?
                {
                    lockfile.inner.insert(name.clone(), old.clone());
                } else {
                    let lockfile = lockfile.clone();
                    let name = name.clone();
                    let pkg = pkg.clone();
                    let span = span.clone();
                    tasks.spawn_local_on(
                        async move {
                            span.pb_inc_length(1);
                            lockfile.fetch(system, name, &pkg).await?;
                            span.pb_inc(1);
                            Result::<_>::Ok(())
                        },
                        &local,
                    );
                }
            }
        }

        local
            .run_until(async {
                while let Some(res) = tasks.join_next().await {
                    res.into_diagnostic()??;
                }
                Result::<_>::Ok(())
            })
            .await?;

        self.lockfile.write_dir(&self.dir)
    }

    pub async fn pull(&self, paths: Vec<StorePath>) -> Result<()> {
        let span = info_span!("pull", indicatif.pb_show = Empty);
        span.pb_set_message("pulling dependencies");
        span.pb_set_length(0);
        span.pb_start();

        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(paths).map_err(|_| miette!("channel closed"))?;

        let mut downloaded = BTreeSet::new();
        let mut tasks = JoinSet::new();
        let worker_style = Arc::new(ProgressStyle::with_template(" ‣ {msg}").into_diagnostic()?);

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

            let caches = &self.manifest.systems[&self.system].caches;
            for path in paths {
                if !downloaded.insert(path.clone()) {
                    continue;
                }

                let caches = caches.clone();
                let span = span.clone();
                let store = self.store.clone();
                let tx = tx.clone();
                let worker_style = worker_style.clone();

                tasks.spawn(async move {
                    if store.path.join(&path).symlink_metadata().is_ok()
                        && let Some(references) = store.get_references(path.hash()).await?
                    {
                        tx.send(references).map_err(|_| miette!("channel closed"))?;
                        return Ok(());
                    }

                    span.pb_inc_length(1);
                    let worker = info_span!("worker", indicatif.pb_show = Empty);
                    worker.pb_set_style(&worker_style);
                    worker.pb_set_message(path.as_str());
                    worker.pb_start();

                    let (cache, narinfo) = query(&path, caches).await?;
                    let nar = cache.join(&narinfo.url).into_diagnostic()?;
                    tx.send(narinfo.references.clone())
                        .map_err(|_| miette!("channel closed"))?;

                    let put_references = store.put_references(path.hash(), &narinfo.references);
                    let unpack_nar = async {
                        let nar = HTTP_CLIENT
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
                    info!("downloaded {path} from {cache}");
                    span.pb_inc(1);
                    Result::<_>::Ok(())
                });
            }
        }

        Ok(())
    }

    pub async fn env(&self) -> Result<Vec<(&str, String)>> {
        let Some(manifest) = self.manifest.systems.get(&self.system) else {
            bail!("system {} not supported by the manifest", self.system);
        };

        let mut paths = self.lockfile.collect_outputs(&self.system);
        self.pull(paths.clone()).await?;
        paths.extend(self.store.propagated_build_inputs(paths.clone()).await?);

        let path = self.store.prefix_env_subpaths("PATH", ":", &paths, "bin")?;

        let library_path = self
            .store
            .prefix_env_subpaths("LIBRARY_PATH", ":", &paths, "lib")?;

        let pkg_config_path =
            self.store
                .prefix_env_subpaths("PKG_CONFIG_PATH", ":", &paths, "lib/pkgconfig")?;

        let mut env = vec![
            ("PATH", path),
            ("LIBRARY_PATH", library_path),
            ("PKG_CONFIG_PATH", pkg_config_path),
        ];

        let mut pkgs = HashMap::new();
        for entry in &self.lockfile.systems[&self.system].inner {
            let (name, pkg) = entry.pair();
            pkgs.extend(pkg.outputs.iter().map(move |(output, path)| {
                (format!("{name}.{output}"), format!("/nix/store/{path}"))
            }));
        }
        for (name, value) in &manifest.env {
            env.push((name.as_ref(), strfmt(value, &pkgs).into_diagnostic()?));
        }

        Ok(env)
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

async fn query(path: &StorePath, caches: Vec<Arc<Url>>) -> Result<(Arc<Url>, Narinfo)> {
    for cache in caches {
        debug!("checking {path} on {cache}");
        match query_one(path.hash(), &cache).await {
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

async fn query_one(hash: &str, cache: &Url) -> Result<Option<String>> {
    let res = HTTP_CLIENT
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

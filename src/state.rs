use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::Cursor,
    process::Command,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use camino::{Utf8Path, Utf8PathBuf};
use harmonia_store_core::signature::PublicKey;
use miette::{IntoDiagnostic, Result, bail, miette};
use reqwest::{Client, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{Jitter, RetryTransientMiddleware, policies::ExponentialBackoff};
use strfmt::strfmt;
use tokio::{select, sync::mpsc, task::JoinSet, try_join};
use tracing::{debug, field::Empty, info, info_span, warn};
use tracing_indicatif::{span_ext::IndicatifSpanExt, style::ProgressStyle};
use url::Url;

use crate::{
    cli::GlobalArgs,
    lockfile::{Lockfile, SystemLockfile},
    manifest::{Manifest, SystemManifest},
    resolver::ResolverJobs,
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
    pub fn new(global: GlobalArgs, system: Option<System>) -> Result<Self> {
        let dir = global.directory.unwrap_or_else(|| ".".into());
        let manifest = Manifest::from_dir(&dir)?;
        let system = match system {
            Some(system) => system,
            None => System::host()?,
        };

        Ok(Self {
            dir,
            lockfile: Lockfile::default(),
            manifest,
            store: Arc::new(Store::new()?),
            system,
        })
    }

    pub async fn new_locked(global: GlobalArgs, system: Option<System>) -> Result<Self> {
        let locked = global.locked;
        let mut state = State::new(global, system)?;
        if locked {
            if !state.locked().await? {
                bail!("cannot update lockfile with --locked");
            }
        } else {
            state.lock().await?;
        }
        Ok(state)
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
        let worker_style = Arc::new(ProgressStyle::with_template("  ‣ {msg}").into_diagnostic()?);

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

            let manifest = &self.manifest.systems[&self.system];
            for path in paths {
                if !downloaded.insert(path.clone()) {
                    continue;
                }

                let caches = manifest.caches.clone();
                let public_keys = manifest.public_keys.clone();
                let span = span.clone();
                let store = self.store.clone();
                let tx = tx.clone();
                let worker_style = worker_style.clone();

                tasks.spawn(async move {
                    let _lock = store.lock_path(&path).await?;

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

                    let (cache, narinfo) = query(&path, caches, &public_keys).await?;
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
                            .unpack_nar(
                                &path,
                                Cursor::new(nar),
                                narinfo.compression,
                                narinfo.nar_hash,
                                narinfo.nar_size,
                            )
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

    pub async fn env(&self) -> Result<BTreeMap<&str, String>> {
        let Some(manifest) = self.manifest.systems.get(&self.system) else {
            bail!("system {} not supported by the manifest", self.system);
        };

        let mut paths = self.lockfile.collect_outputs(&self.system);
        self.pull(paths.clone()).await?;
        paths.extend(self.store.propagated_build_inputs(paths.clone()).await?);

        let (mut env, path) = try_join!(
            self.extra_env(&paths, manifest),
            self.store.prefix_env_subpaths("PATH", ":", &paths, "bin"),
        )?;
        if let Some(path) = path {
            env.entry("PATH").or_insert(path);
        }

        Ok(env)
    }

    // environment variables other than $PATH
    pub async fn extra_env<'a>(
        &self,
        paths: &[StorePath],
        manifest: &'a SystemManifest,
    ) -> Result<BTreeMap<&'a str, String>> {
        let (ld_library_path, library_path, pkg_config_path, pythonpath) = try_join!(
            self.store
                .prefix_env_subpaths("LD_LIBRARY_PATH", ":", paths, "lib"),
            self.store
                .prefix_env_subpaths("LIBRARY_PATH", ":", paths, "lib"),
            self.store
                .prefix_env_subpaths("PKG_CONFIG_PATH", ":", paths, "lib/pkgconfig"),
            self.store.prefix_python_subpaths(paths),
        )?;

        let env = [
            ("LD_LIBRARY_PATH", ld_library_path),
            ("LIBRARY_PATH", library_path),
            ("PKG_CONFIG_PATH", pkg_config_path),
            ("PYTHONPATH", pythonpath),
        ];
        let mut env: BTreeMap<_, _> = env.into_iter().flat_map(|(k, v)| Some((k, v?))).collect();

        let mut pkgs = HashMap::new();
        for entry in &self.lockfile.systems[&self.system].inner {
            let (name, pkg) = entry.pair();
            pkgs.extend(pkg.outputs.iter().map(move |(output, path)| {
                (format!("{name}.{output}"), format!("/nix/store/{path}"))
            }));
        }
        for (name, value) in &manifest.env {
            env.insert(name.as_ref(), strfmt(value, &pkgs).into_diagnostic()?);
        }

        Ok(env)
    }

    pub fn bwrap(&self) -> Result<Command> {
        let mut cmd = Command::new("bwrap");
        for entry in Utf8Path::new("/").read_dir().into_diagnostic()? {
            let path = entry.into_diagnostic()?.path();
            cmd.arg("--bind").arg(&path).arg(&path);
        }

        cmd.arg("--dev-bind").arg("/dev").arg("/dev");
        cmd.arg("--proc").arg("/proc");

        if Utf8Path::new("/nix/store").is_dir() {
            cmd.arg("--overlay-src").arg("/nix/store");
            cmd.arg("--overlay-src").arg(&self.store.path);
            cmd.arg("--ro-overlay").arg("/nix/store");
        } else {
            cmd.arg("--ro-bind").arg(&self.store.path).arg("/nix/store");
        }

        cmd.arg("--");
        Ok(cmd)
    }

    async fn lock(&mut self) -> Result<()> {
        let span = info_span!("lock", indicatif.pb_show = Empty);
        span.pb_set_message("generating lockfile");
        span.pb_set_length(0);
        span.pb_start();
        let mut jobs = ResolverJobs::new(span);

        let mut old = Lockfile::from_dir(&self.dir)?;
        for (&system, manifest) in &self.manifest.systems {
            let lockfile = Rc::new(SystemLockfile::default());
            let old = old.systems.get_mut(&system);
            for (name, pkg) in &manifest.packages {
                let key = pkg.key()?;
                if let Some(old) = &old
                    && let Some((name, old)) = old.inner.remove(name)
                    && old.key == key
                {
                    lockfile.inner.insert(name, old);
                } else {
                    jobs.add(name.clone(), key, pkg, system)?;
                }
            }
            self.lockfile.systems.insert(system, lockfile);
        }

        jobs.resolve(&self.lockfile).await?;
        self.lockfile.write_dir(&self.dir)
    }

    async fn locked(&mut self) -> Result<bool> {
        let mut old = Lockfile::from_dir(&self.dir)?;

        for (&system, manifest) in &self.manifest.systems {
            let Some(old) = old.systems.remove(&system) else {
                return Ok(false);
            };

            let lockfile = SystemLockfile::default();
            for (name, pkg) in &manifest.packages {
                let Some((name, old)) = old.inner.remove(name) else {
                    return Ok(false);
                };
                if old.key != pkg.key()? {
                    return Ok(false);
                }
                lockfile.inner.insert(name, old);
            }

            if !old.inner.is_empty() {
                return Ok(false);
            }

            self.lockfile.systems.insert(system, Rc::new(lockfile));
        }

        Ok(old.systems.is_empty())
    }
}

async fn query(
    path: &StorePath,
    caches: Vec<Arc<Url>>,
    public_keys: &[Arc<PublicKey>],
) -> Result<(Arc<Url>, Narinfo)> {
    for cache in caches {
        debug!("checking {path} on {cache}");
        match query_one(path.hash(), &cache).await {
            Ok(Some(narinfo)) => {
                return Ok((cache, Narinfo::parse(&narinfo, public_keys)?));
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

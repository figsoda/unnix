pub mod nar;
pub mod path;

use std::{
    collections::BTreeSet,
    env::var_os,
    ffi::OsString,
    fs::{create_dir_all, rename},
    io::Cursor,
    num::NonZero,
    time::Duration,
};

use async_compression::tokio::bufread::{
    BrotliDecoder, BzDecoder, GzipDecoder, Lz4Decoder, LzmaDecoder, XzDecoder, ZstdDecoder,
};
use camino::{Utf8Path, Utf8PathBuf};
use dirs::cache_dir;
use fs4::tokio::AsyncFileExt;
use itertools::Itertools;
use miette::{IntoDiagnostic, Result, miette};
use nix_nar::Decoder;
use tempfile::TempDir;
use tokio::{
    fs::File,
    io::{AsyncBufRead, AsyncReadExt, AsyncWriteExt},
    task::{JoinSet, LocalSet, spawn_blocking},
    time::sleep,
};

use crate::store::{nar::Compression, path::StorePath};

#[derive(Clone)]
pub struct Store {
    pub path: Utf8PathBuf,
    lock: Utf8PathBuf,
    references: Utf8PathBuf,
}

impl Store {
    pub fn new() -> Result<Self> {
        let cache = cache_dir().ok_or_else(|| miette!("no cache directory found"))?;
        let cache = Utf8PathBuf::try_from(cache)
            .into_diagnostic()?
            .join("unnix");

        let path = cache.join("store");
        let lock = cache.join("lock");
        let references = cache.join("references");

        create_dir_all(&path).into_diagnostic()?;
        create_dir_all(&lock).into_diagnostic()?;
        create_dir_all(&references).into_diagnostic()?;

        Ok(Store {
            path,
            lock,
            references,
        })
    }

    pub async fn unpack_nar(
        &self,
        path: &StorePath,
        mut reader: impl AsyncBufRead + Unpin,
        compression: Compression,
    ) -> Result<()> {
        let out = self.path.join(path);
        if out.symlink_metadata().is_ok() {
            return Ok(());
        }

        let lock = File::create(self.lock.join(path)).await.into_diagnostic()?;
        while !lock.try_lock_exclusive().into_diagnostic()? {
            sleep(Duration::from_millis(250)).await;
        }

        if out.symlink_metadata().is_ok() {
            return Ok(());
        }

        let mut buf = Vec::new();
        match compression {
            Compression::Brotli => {
                BrotliDecoder::new(reader)
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
            Compression::Bzip2 => {
                BzDecoder::new(reader)
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
            Compression::Gzip => {
                GzipDecoder::new(reader)
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
            Compression::Lz4 => {
                Lz4Decoder::new(reader)
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
            Compression::Lzma => {
                LzmaDecoder::new(reader)
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
            Compression::None => {
                reader.read_to_end(&mut buf).await.into_diagnostic()?;
            }
            Compression::Xz => {
                XzDecoder::parallel(reader, NonZero::new(4).unwrap())
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
            Compression::Zstd => {
                ZstdDecoder::new(reader)
                    .read_to_end(&mut buf)
                    .await
                    .into_diagnostic()?;
            }
        }

        spawn_blocking(|| {
            let tmp = TempDir::with_prefix("unnix-").into_diagnostic()?;
            let tmp = tmp.path().join("out");

            Decoder::new(Cursor::new(buf))
                .into_diagnostic()?
                .unpack(&tmp)
                .into_diagnostic()?;

            rename(tmp, out).into_diagnostic()
        })
        .await
        .into_diagnostic()?
    }

    pub async fn get_references(&self, hash: &str) -> Result<Option<Vec<StorePath>>> {
        let Ok(mut file) = File::open(self.references.join(format!("{hash}.json"))).await else {
            return Ok(None);
        };

        while !file.try_lock_exclusive().into_diagnostic()? {
            sleep(Duration::from_millis(250)).await;
        }

        let mut text = String::new();
        file.read_to_string(&mut text).await.into_diagnostic()?;
        serde_json::from_str(&text).into_diagnostic()
    }

    pub async fn put_references(&self, hash: &str, references: &[StorePath]) -> Result<()> {
        let mut file = File::create(self.references.join(format!("{hash}.json")))
            .await
            .into_diagnostic()?;

        if !file.try_lock_exclusive().into_diagnostic()? {
            return Ok(());
        }

        let buf = serde_json::to_vec(references).into_diagnostic()?;
        file.write_all(&buf).await.into_diagnostic()
    }

    pub async fn propagated_build_inputs(
        &self,
        mut paths: Vec<StorePath>,
    ) -> Result<BTreeSet<StorePath>> {
        let mut propagated = BTreeSet::new();
        let mut checked = BTreeSet::new();

        let local = LocalSet::new();
        let mut tasks = JoinSet::new();

        while !paths.is_empty() {
            for path in paths.drain(..) {
                if !checked.insert(path.clone()) {
                    continue;
                }

                let path = self
                    .path
                    .join(path)
                    .join("nix-support/propagated-build-inputs");

                tasks.spawn_local_on(
                    async move {
                        let Ok(mut file) = File::open(path).await else {
                            return Ok(Vec::new());
                        };

                        let mut text = String::new();
                        file.read_to_string(&mut text).await.into_diagnostic()?;
                        text.split_whitespace().map(StorePath::new).collect()
                    },
                    &local,
                );
            }

            local
                .run_until(async {
                    while let Some(res) = tasks.join_next().await {
                        for path in res.into_diagnostic()?? {
                            paths.push(path.clone());
                            propagated.insert(path);
                        }
                    }
                    Result::<_>::Ok(())
                })
                .await?;
        }

        Ok(propagated)
    }

    pub fn prefix_env_subpaths(
        &self,
        name: &str,
        sep: &str,
        paths: &[StorePath],
        subpath: &str,
    ) -> OsString {
        let mut paths: OsString = paths
            .iter()
            .flat_map(|path| {
                let path = path.as_ref().join(subpath);
                self.path
                    .join(&path)
                    .exists()
                    .then(|| Utf8Path::new("/nix/store").join(path))
            })
            .join(sep)
            .into();

        if let Some(old) = var_os(name) {
            paths.push(sep);
            paths.push(old);
        }

        paths
    }
}

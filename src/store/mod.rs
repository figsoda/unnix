pub mod nar;
pub mod path;

use std::{
    collections::BTreeSet,
    env::{VarError, var},
    fmt::Write,
    fs::create_dir_all,
    io::Cursor,
    num::NonZero,
    pin::pin,
    time::Duration,
};

use async_compression::tokio::bufread::{
    BrotliDecoder, BzDecoder, GzipDecoder, Lz4Decoder, LzmaDecoder, XzDecoder, ZstdDecoder,
};
use async_stream::stream;
use camino::{Utf8Path, Utf8PathBuf};
use dirs::cache_dir;
use fs4::tokio::AsyncFileExt;
use harmonia_utils_hash::{Hash, fmt::CommonHash};
use miette::{IntoDiagnostic, Result, bail, miette};
use nix_nar::Decoder;
use tempfile::{NamedTempFile, TempDir};
use tokio::{
    fs::{File, rename, symlink_metadata},
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    task::{JoinSet, LocalSet, spawn_blocking},
    time::sleep,
};
use tokio_stream::{Stream, StreamExt, wrappers::LinesStream};

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

    pub async fn lock_path(&self, path: &StorePath) -> Result<File> {
        let lock = File::create(self.lock.join(path)).await.into_diagnostic()?;
        while !lock.try_lock_exclusive().into_diagnostic()? {
            sleep(Duration::from_millis(250)).await;
        }
        Ok(lock)
    }

    pub async fn unpack_nar(
        &self,
        path: &StorePath,
        mut reader: impl AsyncBufRead + Unpin,
        compression: Compression,
        nar_hash: Hash,
        nar_size: usize,
    ) -> Result<()> {
        let out = self.path.join(path);
        if out.symlink_metadata().is_ok() {
            return Ok(());
        }

        let mut buf = Vec::with_capacity(nar_size);
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

        let path = path.clone();
        spawn_blocking(move || {
            let actual_hash = nar_hash.algorithm().digest(&buf);
            if actual_hash != nar_hash {
                return Err(miette!(
                    "expected: {}\n  actual: {}",
                    nar_hash.sri(),
                    actual_hash.sri(),
                )
                .wrap_err(format!("nar hash mismatch for {path}")));
            }

            let tmp = TempDir::with_prefix("unnix-").into_diagnostic()?;
            let tmp = tmp.path().join("out");

            Decoder::new(Cursor::new(buf))
                .into_diagnostic()?
                .unpack(&tmp)
                .into_diagnostic()?;

            std::fs::rename(tmp, out).into_diagnostic()
        })
        .await
        .into_diagnostic()?
    }

    pub async fn get_references(&self, hash: &str) -> Result<Option<Vec<StorePath>>> {
        let Ok(file) = File::open(self.references.join(hash)).await else {
            return Ok(None);
        };

        LinesStream::new(BufReader::new(file).lines())
            .map(|line| StorePath::from_storeless(line.into_diagnostic()?))
            .collect::<Result<_>>()
            .await
            .map(Some)
    }

    pub async fn put_references(&self, hash: &str, references: &[StorePath]) -> Result<()> {
        let tmp = spawn_blocking(NamedTempFile::new)
            .await
            .into_diagnostic()?
            .into_diagnostic()?;

        let (file, path) = tmp.into_parts();
        let mut file = File::from_std(file);

        let mut text = String::new();
        for path in references {
            writeln!(text, "{path}").into_diagnostic()?;
        }
        file.write_all(text.as_bytes()).await.into_diagnostic()?;

        rename(&path, self.references.join(hash))
            .await
            .into_diagnostic()?;
        let _ = spawn_blocking(|| path.close()).await;

        Ok(())
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

    pub async fn prefix_env_subpaths(
        &self,
        name: &str,
        sep: &str,
        paths: &[StorePath],
        subpath: &str,
    ) -> Result<String> {
        let mut val = String::new();
        let mut paths = pin!(self.subpaths(paths, subpath).await);
        if let Some(path) = paths.next().await {
            val.push_str(path.as_str());
            while let Some(path) = paths.next().await {
                val.push_str(sep);
                val.push_str(path.as_str());
            }
        }

        match var(name) {
            Ok(old) if val.is_empty() => Ok(old),
            Ok(old) => {
                val.push_str(sep);
                val.push_str(&old);
                Ok(val)
            }
            Err(VarError::NotPresent) => Ok(val),
            Err(e) => {
                bail!(e);
            }
        }
    }

    pub async fn subpaths<'a>(
        &'a self,
        paths: &'a [StorePath],
        subpath: &'a str,
    ) -> impl Stream<Item = Utf8PathBuf> + 'a {
        stream!({
            for path in paths {
                let path = path.as_ref().join(subpath);
                if symlink_metadata(self.path.join(&path)).await.is_ok() {
                    yield Utf8Path::new("/nix/store").join(path);
                }
            }
        })
    }
}

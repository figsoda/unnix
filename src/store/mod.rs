pub mod nar;
pub mod path;

use std::{fs::create_dir_all, io::Cursor, num::NonZero, rc::Rc, time::Duration};

use async_compression::tokio::bufread::{
    BrotliDecoder, BzDecoder, GzipDecoder, Lz4Decoder, LzmaDecoder, XzDecoder, ZstdDecoder,
};
use camino::Utf8PathBuf;
use derive_more::AsRef;
use dirs::cache_dir;
use fs4::tokio::AsyncFileExt;
use miette::{IntoDiagnostic, Result, miette};
use nix_nar::Decoder;
use tokio::{
    fs::File,
    io::{AsyncBufRead, AsyncReadExt, AsyncWriteExt},
    task::spawn_blocking,
    time::sleep,
};

use crate::store::{nar::Compression, path::StorePath};

#[derive(AsRef, Clone)]
pub struct Store {
    #[as_ref(forward)]
    path: Rc<Utf8PathBuf>,
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
            path: Rc::new(path),
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
        if self.contains(path) {
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

        let lock = File::create(self.lock.join(path)).await.into_diagnostic()?;
        while !lock.try_lock_exclusive().into_diagnostic()? {
            sleep(Duration::from_millis(250)).await;
        }

        let path = self.path.join(path);
        if path.exists() {
            return Ok(());
        }

        spawn_blocking(|| {
            Decoder::new(Cursor::new(buf))
                .into_diagnostic()?
                .unpack(path)
                .into_diagnostic()
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

    pub fn contains(&self, path: &StorePath) -> bool {
        self.path.join(path).exists()
    }
}

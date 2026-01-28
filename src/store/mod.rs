pub mod nar;
pub mod path;

use std::{fs::create_dir_all, io::Cursor, num::NonZero};

use async_compression::tokio::bufread::{
    BrotliDecoder, BzDecoder, GzipDecoder, Lz4Decoder, LzmaDecoder, XzDecoder, ZstdDecoder,
};
use camino::Utf8PathBuf;
use dirs::cache_dir;
use miette::{IntoDiagnostic, Result, miette};
use nix_nar::Decoder;
use tokio::io::{AsyncBufRead, AsyncReadExt};

use crate::store::{nar::Compression, path::StorePath};

pub struct Store {
    pub path: Utf8PathBuf,
}

impl Store {
    pub fn new() -> Result<Self> {
        let cache = cache_dir().ok_or_else(|| miette!("no cache directory found"))?;
        let path = Utf8PathBuf::try_from(cache)
            .into_diagnostic()?
            .join("unnix/store");
        create_dir_all(&path).into_diagnostic()?;
        Ok(Store { path })
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

        Decoder::new(Cursor::new(buf))
            .into_diagnostic()?
            .unpack(self.path.join(path))
            .into_diagnostic()?;

        Ok(())
    }

    pub fn contains(&self, path: &StorePath) -> bool {
        self.path.join(path).exists()
    }
}

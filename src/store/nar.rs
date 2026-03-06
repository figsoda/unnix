use std::{fmt::Write, str::FromStr, sync::Arc};

use harmonia_store_core::signature::PublicKey;
use harmonia_utils_hash::{Hash, fmt::Any};
use miette::{IntoDiagnostic, Report, Result, WrapErr, bail};

use crate::store::path::StorePath;

#[derive(Debug)]
pub struct Narinfo {
    pub compression: Compression,
    pub nar_hash: Hash,
    pub nar_size: usize,
    pub references: Vec<StorePath>,
    pub url: String,
}

#[derive(Debug)]
pub enum Compression {
    Brotli,
    Bzip2,
    Gzip,
    Lz4,
    Lzma,
    None,
    Xz,
    Zstd,
}

impl Narinfo {
    pub fn parse(content: &str, public_keys: &[Arc<PublicKey>]) -> Result<Self> {
        let mut compression = None;
        let mut nar_hash = None;
        let mut nar_size = None;
        let mut references = None;
        let mut sig = None;
        let mut store_path = None;
        let mut url = None;

        for (key, value) in content.lines().flat_map(|line| line.split_once(": ")) {
            match key {
                "Compression" => {
                    compression = Some(value.parse()?);
                }
                "NarHash" => {
                    nar_hash = Some(value);
                }
                "NarSize" => {
                    nar_size = Some(value.parse().into_diagnostic()?);
                }
                "References" => {
                    references = Some(
                        value
                            .split_whitespace()
                            .map(StorePath::from_storeless)
                            .collect::<Result<_>>()?,
                    );
                }
                "Sig" => {
                    sig = Some(value.parse().into_diagnostic()?);
                }
                "StorePath" => {
                    store_path = Some(StorePath::new(value)?);
                }
                "URL" => {
                    url = Some(value.to_owned());
                }
                _ => {}
            }
        }

        let compression = compression.wrap_err("Compression missing in narinfo")?;
        let nar_hash = nar_hash.wrap_err("NarHash missing in narinfo")?;
        let nar_size = nar_size.wrap_err("NarSize missing in narinfo")?;
        let mut references: Vec<_> = references.wrap_err("References missing in narinfo")?;
        let sig = sig.wrap_err("Sig missing in narinfo")?;
        let store_path = store_path.wrap_err("StorePath missing in narinfo")?;
        let url = url.wrap_err("URL missing in narinfo")?;

        references.sort();
        let mut fingerprint = format!("1;/nix/store/{store_path};{nar_hash};{nar_size};");
        let mut paths = references.iter();
        if let Some(path) = paths.next() {
            write!(fingerprint, "/nix/store/{path}").into_diagnostic()?;
            for path in paths {
                write!(fingerprint, ",/nix/store/{path}").into_diagnostic()?;
            }
        }

        if public_keys.iter().any(|pk| pk.verify(&fingerprint, &sig)) {
            Ok(Self {
                compression,
                nar_hash: nar_hash.parse::<Any<_>>().into_diagnostic()?.into(),
                nar_size,
                references,
                url,
            })
        } else {
            bail!("failed to verify {store_path}");
        }
    }
}

impl FromStr for Compression {
    type Err = Report;

    fn from_str(fmt: &str) -> Result<Self> {
        match fmt {
            "br" => Ok(Compression::Brotli),
            "bzip2" => Ok(Compression::Bzip2),
            "gzip" => Ok(Compression::Gzip),
            "lz4" => Ok(Compression::Lz4),
            "lzma" => Ok(Compression::Lzma),
            "none" => Ok(Compression::None),
            "xz" => Ok(Compression::Xz),
            "zstd" => Ok(Compression::Zstd),
            _ => bail!("unsupported compression format: {fmt:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use insta::assert_debug_snapshot;

    use super::Narinfo;
    use crate::manifest::DEFAULT_PUBLIC_KEY;

    #[test]
    fn basic() {
        let content = "
StorePath: /nix/store/hwz2l7ihv2skq7gr5l3paavs3rr9il7z-hello-2.12.1
URL: nar/0h9dh04gd4zj0f4wcfn0i6f496q054fs3fpw099x5mcdayzi6ra6.nar.xz
Compression: xz
FileHash: sha256:0h9dh04gd4zj0f4wcfn0i6f496q054fs3fpw099x5mcdayzi6ra6
FileSize: 50356
NarHash: sha256:1kcsbgcx1f2z7qaj4a29zfa8ad7866f15hdbcds6kv92qf928fkw
NarSize: 226560
References: 5m9amsvvh2z8sl7jrnc87hzy21glw6k1-glibc-2.40-66 hwz2l7ihv2skq7gr5l3paavs3rr9il7z-hello-2.12.1
Deriver: gciipqhqkdlqqn803zd4a389v86ran45-hello-2.12.1.drv
Sig: cache.nixos.org-1:k2IFtC1gRLHfYPqHVmOUI2leueaS6DLXlmiQSsp2tOJ4+kKdx5UAm2m10cR/vz7U50QvgEcvrqCICw2CRLy3Cg==
";
        let pk = Arc::new(DEFAULT_PUBLIC_KEY.parse().unwrap());
        assert_debug_snapshot!(Narinfo::parse(content, &[pk]));
    }
}

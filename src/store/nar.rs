use std::str::FromStr;

use miette::{Report, Result, bail};

use crate::store::path::StorePath;

#[derive(Debug)]
pub struct Narinfo {
    pub compression: Compression,
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
    pub fn parse(content: &str) -> Result<Self> {
        let mut compression = None;
        let mut references = None;
        let mut url = None;

        for (key, value) in content.lines().flat_map(|line| line.split_once(": ")) {
            match key {
                "Compression" => {
                    compression = Some(value.parse()?);
                }
                "References" => {
                    references = Some(
                        value
                            .split_whitespace()
                            .map(StorePath::from_storeless)
                            .collect::<Result<_>>()?,
                    );
                }
                "URL" => {
                    url = Some(value.to_owned());
                }
                _ => {}
            }
        }

        if let (Some(compression), Some(references), Some(url)) = (compression, references, url) {
            Ok(Self {
                compression,
                references,
                url,
            })
        } else {
            bail!("not all required fields found");
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
    use insta::assert_debug_snapshot;

    use super::Narinfo;

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
        assert_debug_snapshot!(Narinfo::parse(content));
    }
}

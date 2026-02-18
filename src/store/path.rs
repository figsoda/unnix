use std::sync::{Arc, LazyLock};

use camino::Utf8Path;
use miette::{Result, bail, miette};
use parse_display::Display;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Display, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StorePath(Arc<str>);

impl StorePath {
    pub fn new(path: &str) -> Result<Self> {
        path.strip_prefix("/nix/store/")
            .ok_or_else(|| miette!("invalid path {path:?}"))
            .and_then(StorePath::from_storeless)
    }

    pub fn from_storeless(path: impl Into<Arc<str>>) -> Result<Self> {
        static REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new("^[0-9abcdfghijklmnpqrsvwxyz]{32}-[^/]+$").unwrap());

        let path = path.into();
        if REGEX.is_match(&path) {
            Ok(Self(path))
        } else {
            bail!("invalid path {path:?}");
        }
    }

    pub fn hash(&self) -> &str {
        &self.0[.. 32]
    }
}

impl AsRef<Utf8Path> for StorePath {
    fn as_ref(&self) -> &Utf8Path {
        Utf8Path::new(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::StorePath;

    #[test]
    fn basic() {
        assert_debug_snapshot!(StorePath::new(
            "/nix/store/hwz2l7ihv2skq7gr5l3paavs3rr9il7z-hello-2.12.1",
        ));
        assert_debug_snapshot!(StorePath::from_storeless(
            "hwz2l7ihv2skq7gr5l3paavs3rr9il7z-hello-2.12.1",
        ));
    }

    #[test]
    fn fails() {
        assert!(
            StorePath::new("/guix/store/hwz2l7ihv2skq7gr5l3paavs3rr9il7z-hello-2.12.1").is_err(),
        );
        assert!(StorePath::new("/nix/store/hello-2.12.1").is_err());
        assert!(StorePath::from_storeless("hwz2l7ihv2skq7gr5l3paavs3rr9il7z").is_err());
    }
}

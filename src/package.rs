use std::{
    collections::BTreeSet,
    fmt::{self, Display, Formatter},
    rc::Rc,
    str::FromStr,
};

use blake3::Hasher;
use data_encoding::BASE64;
use miette::{IntoDiagnostic, Report, Result, miette};
use serde::Serialize;

use crate::source::Source;

#[derive(Debug, Serialize)]
pub struct Package {
    pub attribute: Rc<str>,
    pub outputs: BTreeSet<String>,
    pub source: Rc<Source>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Base64Hash {
    inner: [u8; 32],
}

impl Package {
    pub fn hash(&self) -> Result<Base64Hash> {
        let mut hasher = Hasher::new();
        serde_json::to_writer(&mut hasher, self).into_diagnostic()?;
        Ok(Base64Hash {
            inner: hasher.finalize().into(),
        })
    }
}

impl Display for Base64Hash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", BASE64.encode_display(&self.inner))
    }
}

impl FromStr for Base64Hash {
    type Err = Report;

    fn from_str(hash: &str) -> Result<Self, Self::Err> {
        let inner = BASE64
            .decode(hash.as_bytes())
            .into_diagnostic()?
            .try_into()
            .map_err(|bytes: Vec<u8>| {
                miette!("invalid hash length {}, expected 32", bytes.len())
            })?;
        Ok(Self { inner })
    }
}

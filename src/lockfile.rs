use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    rc::Rc,
};

use camino::Utf8Path;
use dashmap::DashMap;
use miette::{Diagnostic, IntoDiagnostic, NamedSource, Report, Result, SourceOffset};
use monostate::MustBe;
use serde::{Deserialize, Serialize};
use serde_json::{Serializer, ser::PrettyFormatter};
use serde_with::{DisplayFromStr, serde_as};
use thiserror::Error;

use crate::{
    package::{Base64Hash, Package},
    source::GetOutputs,
    store::path::StorePath,
    system::System,
};

type Version = MustBe!(0u64);

#[serde_as]
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    version: Version,
    #[serde_as(as = "BTreeMap<DisplayFromStr, _>")]
    pub systems: BTreeMap<System, Rc<SystemLockfile>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SystemLockfile {
    #[serde(flatten)]
    pub inner: DashMap<Rc<str>, PackageLock>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageLock {
    #[serde_as(as = "DisplayFromStr")]
    pub hash: Base64Hash,
    pub outputs: BTreeMap<String, StorePath>,
}

impl Lockfile {
    pub fn from_dir(path: &Utf8Path) -> Result<Self> {
        let path = path.join("unnix.lock.json");
        let Ok(mut file) = File::open(&path) else {
            return Ok(Self::default());
        };

        let mut text = String::new();
        file.read_to_string(&mut text).into_diagnostic()?;

        serde_json::from_str(&text).map_err(|error| {
            let location = SourceOffset::from_location(&text, error.line(), error.column());
            let mut error = error.to_string();
            if let Some(i) = error.rfind(" at line ") {
                error.truncate(i);
            }
            Report::new(
                #[allow(unused_assignments)]
                {
                    #[derive(Debug, Diagnostic, Error)]
                    #[error("failed to parse JSON file")]
                    struct SerdeJsonError {
                        error: String,
                        #[source_code]
                        file: NamedSource<String>,
                        #[label("{error}")]
                        location: SourceOffset,
                    }
                    SerdeJsonError {
                        error,
                        file: NamedSource::new(path, text),
                        location,
                    }
                },
            )
        })
    }

    pub fn write_dir(&self, path: &Utf8Path) -> Result<()> {
        let mut file = File::create(path.join("unnix.lock.json")).into_diagnostic()?;
        let mut ser = Serializer::with_formatter(&mut file, PrettyFormatter::with_indent(b" "));
        self.serialize(&mut ser).into_diagnostic()?;
        writeln!(file).into_diagnostic()?;
        Ok(())
    }

    pub fn collect_outputs(&self, system: &System) -> Vec<StorePath> {
        let mut outputs = Vec::new();
        if let Some(packages) = self.systems.get(system) {
            for pkg in &packages.inner {
                outputs.extend(pkg.value().outputs.values().cloned());
            }
        }
        outputs
    }
}

impl SystemLockfile {
    pub async fn fetch(&self, system: System, name: Rc<str>, pkg: &Package) -> Result<()> {
        let mut outputs = pkg.source.get_outputs(&pkg.attribute, system).await?;
        if !pkg.outputs.is_empty() {
            outputs.retain(|name, _| pkg.outputs.contains(name.as_str()));
        }

        self.inner.insert(
            name,
            PackageLock {
                hash: pkg.hash()?,
                outputs,
            },
        );

        Ok(())
    }
}

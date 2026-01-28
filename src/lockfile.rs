use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    rc::Rc,
};

use camino::Utf8Path;
use derive_more::{Deref, DerefMut};
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
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub systems: HashMap<System, Packages>,
}

#[derive(Debug, Deref, DerefMut, Default, Deserialize, Serialize)]
pub struct Packages {
    packages: HashMap<String, PackageLock>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageLock {
    #[serde_as(as = "DisplayFromStr")]
    hash: Base64Hash,
    pub outputs: HashMap<String, Rc<StorePath>>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("failed to parse JSON file")]
struct SerdeJsonError {
    error: String,
    #[source_code]
    file: NamedSource<String>,
    #[label("{error}")]
    location: SourceOffset,
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
            Report::new(SerdeJsonError {
                error,
                file: NamedSource::new(path, text),
                location,
            })
        })
    }

    pub fn write_dir(&self, path: &Utf8Path) -> Result<()> {
        let mut file = File::create(path.join("unnix.lock.json")).into_diagnostic()?;
        let mut ser = Serializer::with_formatter(&mut file, PrettyFormatter::with_indent(b" "));
        self.serialize(&mut ser).into_diagnostic()?;
        writeln!(file).into_diagnostic()?;
        Ok(())
    }

    pub async fn add(
        &mut self,
        lockfile: &Lockfile,
        system: System,
        name: String,
        pkg: &Package,
    ) -> Result<()> {
        let hash = pkg.hash()?;
        if let Some(pkg) = lockfile.get(system, &name)
            && pkg.hash == hash
        {
            self.systems
                .entry(system)
                .or_default()
                .insert(name, pkg.clone());
            Ok(())
        } else {
            self.fetch(system, name, pkg).await
        }
    }

    pub async fn fetch(&mut self, system: System, name: String, pkg: &Package) -> Result<()> {
        let mut outputs = pkg.source.get_outputs(&pkg.attribute, system).await?;
        if let Some(names) = &pkg.outputs {
            outputs.retain(|name, _| names.contains(name));
        }

        self.systems.entry(system).or_default().insert(
            name,
            PackageLock {
                hash: pkg.hash()?,
                outputs,
            },
        );

        Ok(())
    }

    pub fn get(&self, system: System, name: &str) -> Option<&PackageLock> {
        self.systems.get(&system)?.get(name)
    }
}

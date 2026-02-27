mod tests;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::read_to_string,
    rc::Rc,
    str::FromStr,
    sync::Arc,
};

use camino::Utf8Path;
use kdl::{KdlDocument, KdlNode};
use miette::{Context, Diagnostic, IntoDiagnostic, Report, Result, SourceSpan, miette};
use thiserror::Error;
use url::Url;

use crate::{
    package::Package,
    source::{Source, hydra::Jobset},
    system::{Arch, Kernel, System},
};

#[derive(Debug)]
pub struct Manifest {
    pub systems: BTreeMap<System, SystemManifest>,
}

#[derive(Clone, Debug)]
pub struct SystemManifest {
    pub packages: BTreeMap<Rc<str>, Rc<Package>>,
    pub caches: Vec<Arc<Url>>,
    pub env: BTreeMap<Rc<str>, Rc<str>>,
}

struct SurfaceSystemManifest<'a> {
    packages: BTreeMap<Rc<str>, SurfacePackage<'a>>,
    env: BTreeMap<Rc<str>, Rc<str>>,
    caches: Vec<Arc<Url>>,
    default_cache: Option<bool>,
    sources: BTreeMap<&'a str, Rc<Source>>,
}

#[derive(Clone)]
struct SurfacePackage<'a> {
    source: &'a str,
    attribute: Rc<str>,
    outputs: BTreeSet<Rc<str>>,
}

struct SystemPredicate {
    pub arch: Option<Arch>,
    pub kernel: Option<Kernel>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("failed to parse manifest file")]
struct ManifestError {
    message: String,
    #[source_code]
    input: String,
    #[label("{message}")]
    span: SourceSpan,
}

macro_rules! kdl_macros {
    // workaround to escape `$`
    // so it can be used with nested macro_rules
    ($inputs:ident) => {
        kdl_macros!(($) $inputs);
    };

    (($_:tt) $input:ident) => {
        macro_rules! err {
            ($node:ident, $_($msg:tt)+) => {
                ManifestError {
                    message: format!($_($msg)*),
                    input: $input.into(),
                    span: $node.span(),
                }
            };
        }

        #[allow(unused_macros)]
        macro_rules! bail {
            ($node:ident, $_($msg:tt)+) => {
                return Err(Report::new(err!($node, $_($msg)*)));
            };
        }

        #[allow(unused_macros)]
        macro_rules! arg {
            ($node:ident) => {
                if let [entry] = $node.entries() {
                    if entry.name().is_some() {
                        bail!($node, "unexpected property");
                    } else {
                        entry
                    }
                } else {
                    bail!($node, "expected one argument");
                }
            };
        }

        #[allow(unused_macros)]
        macro_rules! assert_no_entries {
            ($node:ident) => {
                if let Some(entry) = $node.entries().first() {
                    if entry.name().is_some() {
                        bail!(entry, "unexpected property");
                    } else {
                        bail!(entry, "unexpected argument");
                    }
                }
            };
        }

        #[allow(unused_macros)]
        macro_rules! assert_no_children {
            ($node:ident) => {
                if let Some(child) = $node.children() {
                    bail!(child, "unexpected children");
                }
            };
        }

        #[allow(unused_macros)]
        macro_rules! str {
            ($entry:ident) => {
                $entry
                    .value()
                    .as_string()
                    .wrap_err_with(|| err!($entry, "expected string"))?
            };
        }

        #[allow(unused_macros)]
        macro_rules! str_arg {
            ($node:ident) => {{
                let arg = arg!($node);
                str!(arg)
            }};
        }
    }
}

impl Manifest {
    pub fn from_dir(path: &Utf8Path) -> Result<Self> {
        let path = path.join("unnix.kdl");
        let text = read_to_string(&path).into_diagnostic()?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self> {
        let doc = text.parse()?;
        let mut systems = Vec::new();
        let mut manifests = Vec::new();

        let default = SurfaceSystemManifest::from_document(text, &doc, |node| {
            kdl_macros!(text);

            match node.name().value() {
                "systems" => {
                    for child in node.iter_children() {
                        assert_no_entries!(child);
                        assert_no_children!(child);

                        let name = child.name();
                        match name.value().parse() {
                            Ok(system) => {
                                systems.push(system);
                            }
                            Err(e) => {
                                bail!(name, "{e}");
                            }
                        }
                    }

                    Ok(true)
                }

                "system" => {
                    let entry = arg!(node);
                    let system = str!(entry);
                    let system = if let Ok(system) = System::from_str(system) {
                        SystemPredicate {
                            arch: Some(system.arch),
                            kernel: Some(system.kernel),
                        }
                    } else if let Ok(arch) = Arch::from_str(system) {
                        SystemPredicate {
                            arch: Some(arch),
                            kernel: None,
                        }
                    } else if let Ok(kernel) = Kernel::from_str(system) {
                        SystemPredicate {
                            arch: None,
                            kernel: Some(kernel),
                        }
                    } else {
                        bail!(entry, "unsupported system: {system:?}");
                    };

                    let doc = node
                        .children()
                        .wrap_err_with(|| err!(node, "expected children"))?;
                    let manifest = SurfaceSystemManifest::from_document(text, doc, |_| Ok(false))?;
                    manifests.push((system, manifest));

                    Ok(true)
                }

                _ => Ok(false),
            }
        })?;

        if systems.is_empty() {
            systems.extend([
                System {
                    arch: Arch::Aarch64,
                    kernel: Kernel::Darwin,
                },
                System {
                    arch: Arch::Aarch64,
                    kernel: Kernel::Linux,
                },
                System {
                    arch: Arch::X86_64,
                    kernel: Kernel::Linux,
                },
            ]);
        }

        let default_cache = default.default_cache.unwrap_or(true);
        let mut default_cache: BTreeMap<_, _> = systems
            .iter()
            .map(|system| (*system, default_cache))
            .collect();

        let mut sources = default.sources;
        sources.entry("default").or_default();

        let default = SystemManifest {
            packages: default
                .packages
                .into_iter()
                .map(|(name, pkg)| Ok((name, Package::from_surface(pkg, &sources)?)))
                .collect::<Result<_>>()?,
            caches: default.caches,
            env: default.env,
        };

        let mut systems: BTreeMap<_, _> = systems
            .into_iter()
            .map(|system| (system, default.clone()))
            .collect();

        for (predicate, surface) in manifests {
            for (system, manifest) in systems.iter_mut() {
                if let Some(arch) = predicate.arch
                    && arch != system.arch
                {
                    continue;
                }

                if let Some(kernel) = predicate.kernel
                    && kernel != system.kernel
                {
                    continue;
                }

                let mut sources = sources.clone();
                sources.extend(surface.sources.clone());

                for (name, pkg) in &surface.packages {
                    let pkg = Package::from_surface(pkg.clone(), &sources)?;
                    manifest.packages.insert(name.clone(), pkg);
                }

                manifest.env.extend(
                    surface
                        .env
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone())),
                );

                if let Some(new) = surface.default_cache {
                    default_cache.insert(*system, new);
                }
                manifest.caches.extend(surface.caches.iter().cloned());
            }
        }

        let cache = Arc::new(Url::parse("https://cache.nixos.org").into_diagnostic()?);
        for (system, default_cache) in default_cache {
            if default_cache && let Some(manifest) = systems.get_mut(&system) {
                manifest.caches.insert(0, cache.clone());
            }
        }

        Ok(Manifest { systems })
    }
}

impl<'a> SurfaceSystemManifest<'a> {
    fn from_document(
        text: &str,
        doc: &'a KdlDocument,
        mut handle_unknown: impl FnMut(&'a KdlNode) -> Result<bool>,
    ) -> Result<Self> {
        kdl_macros!(text);

        let mut packages = BTreeMap::new();
        let mut env = BTreeMap::new();
        let mut caches = Vec::new();
        let mut default_cache = None;
        let mut sources = BTreeMap::new();

        for node in doc.nodes() {
            let name = node.name();
            match name.value() {
                "packages" => {
                    assert_no_entries!(node);

                    for child in node.iter_children() {
                        assert_no_children!(child);

                        let mut source = None;
                        let mut attribute = None;
                        let mut outputs = BTreeSet::new();

                        for entry in child.entries() {
                            if let Some(name) = entry.name() {
                                match name.value() {
                                    "source" => {
                                        source = Some(str!(entry));
                                    }
                                    "attribute" => {
                                        attribute = Some(str!(entry));
                                    }
                                    _ => {
                                        bail!(entry, "invalid property");
                                    }
                                }
                            } else if !outputs.insert(str!(entry).into()) {
                                bail!(entry, "duplicate output");
                            }
                        }

                        let pkg = SurfacePackage {
                            source: source.unwrap_or("default"),
                            attribute: attribute.unwrap_or(child.name().value()).into(),
                            outputs,
                        };
                        if packages.insert(child.name().value().into(), pkg).is_some() {
                            bail!(child, "duplicate package");
                        }
                    }
                }

                "env" => {
                    assert_no_entries!(node);

                    for child in node.iter_children() {
                        assert_no_children!(child);
                        let name = str_arg!(child);
                        env.insert(child.name().value().into(), name.into());
                    }
                }

                "caches" => {
                    for entry in node.entries() {
                        if let Some(name) = entry.name() {
                            if name.value() == "default" {
                                default_cache = Some(
                                    entry
                                        .value()
                                        .as_bool()
                                        .wrap_err_with(|| err!(entry, "expected boolean"))?,
                                );
                            } else {
                                bail!(name, "invalid property");
                            }
                        } else {
                            bail!(entry, "unexpected argument");
                        }
                    }

                    for child in node.iter_children() {
                        assert_no_entries!(child);
                        assert_no_children!(child);

                        let name = child.name();
                        match name.value().parse() {
                            Ok(cache) => {
                                caches.push(Arc::new(cache));
                            }
                            Err(e) => {
                                bail!(name, "{e}");
                            }
                        }
                    }
                }

                "hydra" => {
                    let entry = arg!(node);
                    let name = str!(entry);

                    let mut base = None;
                    let mut project = None;
                    let mut jobset = None;
                    let mut job = "{attribute}.{system}";

                    for child in node.iter_children() {
                        assert_no_children!(child);

                        let name = child.name();
                        match name.value() {
                            "base" => {
                                base = Some(str_arg!(child));
                            }
                            "project" => {
                                project = Some(str_arg!(child));
                            }
                            "jobset" => {
                                jobset = Some(str_arg!(child));
                            }
                            "job" => {
                                job = str_arg!(child);
                            }
                            _ => {
                                bail!(entry, "invalid field");
                            }
                        }
                    }

                    let source = Source::Hydra(Jobset {
                        base: base.wrap_err_with(|| err!(node, "missing base"))?.into(),
                        project: project
                            .wrap_err_with(|| err!(node, "missing project"))?
                            .into(),
                        jobset: jobset
                            .wrap_err_with(|| err!(node, "missing jobset"))?
                            .into(),
                        job: job.into(),
                    });
                    if sources.insert(name, source.into()).is_some() {
                        bail!(node, "duplicate source");
                    }
                }

                _ => {
                    if !handle_unknown(node)? {
                        bail!(name, "invalid node");
                    }
                }
            }
        }

        Ok(Self {
            packages,
            env,
            caches,
            default_cache,
            sources,
        })
    }
}

impl Package {
    fn from_surface(
        pkg: SurfacePackage,
        sources: &BTreeMap<&str, Rc<Source>>,
    ) -> Result<Rc<Package>> {
        Ok(Rc::new(Package {
            attribute: pkg.attribute,
            outputs: pkg.outputs,
            source: sources
                .get(pkg.source)
                .ok_or_else(|| miette!("source {:?} not found", pkg.source))?
                .clone(),
        }))
    }
}

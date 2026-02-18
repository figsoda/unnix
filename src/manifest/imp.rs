use std::{collections::BTreeSet, str::FromStr};

use knus::Decode;
use miette::{Report, bail};
use url::Url;

use crate::system::{Arch, Kernel, System};

#[derive(Decode)]
pub struct Manifest {
    #[knus(child, default)]
    pub systems: Systems,

    #[knus(children(name = "system"))]
    pub system: Vec<SystemManifest>,

    #[knus(child, default)]
    pub packages: Packages,

    #[knus(child, default)]
    pub caches: Caches,

    #[knus(child, default)]
    pub env: Env,

    #[knus(children)]
    pub sources: Vec<Source>,
}

#[derive(Decode)]
pub struct Systems {
    #[knus(children)]
    pub inner: Vec<SystemInner>,
}

#[derive(Decode)]
pub struct SystemInner {
    #[knus(node_name)]
    pub inner: System,
}

#[derive(Decode)]
pub struct SystemManifest {
    #[knus(argument, str)]
    pub system: SystemPredicate,

    #[knus(child, default)]
    pub packages: Packages,

    #[knus(child, default)]
    pub caches: Caches,

    #[knus(child, default)]
    pub env: Env,

    #[knus(children)]
    pub sources: Vec<Source>,
}

pub struct SystemPredicate {
    pub arch: Option<Arch>,
    pub kernel: Option<Kernel>,
}

#[derive(Decode, Default)]
pub struct Packages {
    #[knus(children)]
    pub inner: Vec<Package>,
}

#[derive(Clone, Decode)]
pub struct Package {
    #[knus(node_name)]
    pub name: String,
    #[knus(property)]
    pub attribute: Option<String>,
    #[knus(property, default = "default".into())]
    pub source: String,
    #[knus(arguments)]
    pub outputs: BTreeSet<String>,
}

#[derive(Decode)]
pub struct Caches {
    #[knus(property, default = true)]
    pub default: bool,
    #[knus(children)]
    pub inner: Vec<Cache>,
}

#[derive(Clone, Decode)]
pub struct Cache {
    #[knus(node_name)]
    pub url: Url,
}

#[derive(Decode, Default)]
pub struct Env {
    #[knus(children)]
    pub inner: Vec<Var>,
}

#[derive(Clone, Decode)]
pub struct Var {
    #[knus(node_name)]
    pub name: String,
    #[knus(argument)]
    pub value: String,
}

#[derive(Clone, Decode)]
pub enum Source {
    Hydra(Hydra),
}

#[derive(Clone, Decode)]
pub struct Hydra {
    #[knus(argument)]
    pub name: String,
    #[knus(child, unwrap(argument))]
    pub base: String,
    #[knus(child, unwrap(argument))]
    pub project: String,
    #[knus(child, unwrap(argument))]
    pub jobset: String,
    #[knus(child, default = "{attribute}.{system}".into(), unwrap(argument))]
    pub job: String,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            default: true,
            inner: Vec::new(),
        }
    }
}

impl Default for Systems {
    fn default() -> Self {
        Self {
            inner: vec![
                SystemInner {
                    inner: System {
                        arch: Arch::Aarch64,
                        kernel: Kernel::Darwin,
                    },
                },
                SystemInner {
                    inner: System {
                        arch: Arch::Aarch64,
                        kernel: Kernel::Linux,
                    },
                },
                SystemInner {
                    inner: System {
                        arch: Arch::X86_64,
                        kernel: Kernel::Linux,
                    },
                },
            ],
        }
    }
}

impl FromStr for SystemPredicate {
    type Err = Report;

    fn from_str(system: &str) -> Result<Self, Self::Err> {
        if let Ok(system) = System::from_str(system) {
            Ok(Self {
                arch: Some(system.arch),
                kernel: Some(system.kernel),
            })
        } else if let Ok(arch) = Arch::from_str(system) {
            Ok(Self {
                arch: Some(arch),
                kernel: None,
            })
        } else if let Ok(kernel) = Kernel::from_str(system) {
            Ok(Self {
                arch: None,
                kernel: Some(kernel),
            })
        } else {
            bail!("unsupported system: {system:?}");
        }
    }
}

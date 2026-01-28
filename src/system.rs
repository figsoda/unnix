use std::{
    env::consts::{ARCH, OS},
    str::FromStr,
};

use derive_more::Display;
use miette::{Report, Result, bail};

#[derive(Clone, Copy, Debug, Display, Eq, Hash, PartialEq)]
#[display("{arch}-{kernel}")]
pub struct System {
    pub arch: Arch,
    pub kernel: Kernel,
}

pub struct SystemPredicate {
    pub architecture: Option<Arch>,
    pub kernel: Option<Kernel>,
}

#[derive(Clone, Copy, Debug, Display, Eq, Hash, PartialEq)]
#[display(rename_all = "lowercase")]
pub enum Arch {
    Aarch64,
    #[display("x86_64")]
    X86_64,
}

#[derive(Clone, Copy, Debug, Display, Eq, Hash, PartialEq)]
#[display(rename_all = "lowercase")]
pub enum Kernel {
    Darwin,
    Linux,
}

impl FromStr for System {
    type Err = Report;

    fn from_str(system: &str) -> Result<Self, Self::Err> {
        match system {
            "aarch64-darwin" => Ok(System {
                arch: Arch::Aarch64,
                kernel: Kernel::Darwin,
            }),
            "aarch64-linux" => Ok(System {
                arch: Arch::Aarch64,
                kernel: Kernel::Linux,
            }),
            "x86_64-darwin" => Ok(System {
                arch: Arch::X86_64,
                kernel: Kernel::Darwin,
            }),
            "x86_64-linux" => Ok(System {
                arch: Arch::X86_64,
                kernel: Kernel::Linux,
            }),
            _ => {
                bail!("unsupported system: {system:?}");
            }
        }
    }
}

impl System {
    pub fn host() -> Result<Self> {
        let arch = match ARCH {
            "aarch64" => Arch::Aarch64,
            "x86_64" => Arch::X86_64,
            _ => {
                bail!("unsupported architecture: {ARCH}");
            }
        };

        let kernel = match OS {
            "linux" => Kernel::Linux,
            "macos" => Kernel::Darwin,
            _ => {
                bail!("unsupported operating system: {OS}");
            }
        };

        Ok(Self { arch, kernel })
    }
}

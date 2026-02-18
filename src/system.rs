use std::env::consts::{ARCH, OS};

use miette::{Result, bail};
use parse_display::{Display, FromStr};

#[derive(Clone, Copy, Debug, Display, Eq, FromStr, PartialEq, PartialOrd, Ord)]
#[display("{arch}-{kernel}")]
pub struct System {
    pub arch: Arch,
    pub kernel: Kernel,
}

#[derive(Clone, Copy, Debug, Display, Eq, FromStr, PartialEq, PartialOrd, Ord)]
#[display(style = "lowercase")]
pub enum Arch {
    Aarch64,
    #[display("x86_64")]
    X86_64,
}

#[derive(Clone, Copy, Debug, Display, Eq, FromStr, PartialEq, PartialOrd, Ord)]
#[display(style = "lowercase")]
pub enum Kernel {
    Darwin,
    Linux,
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

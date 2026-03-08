use std::path::{Path, PathBuf};

use assert_cmd::{Command, cargo::cargo_bin_cmd};
use fs_extra::dir::{CopyOptions, copy};
use tempfile::TempDir;

pub struct TestEnv {
    fixture: PathBuf,
    tmp: TempDir,
}

impl TestEnv {
    pub fn new(fixture: &str) -> Self {
        let tmp = TempDir::new().unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);

        copy(&fixture, tmp.path(), &CopyOptions::new().content_only(true)).unwrap();
        Self { tmp, fixture }
    }

    pub fn command(&self) -> Command {
        let mut cmd = cargo_bin_cmd!();
        cmd.current_dir(self.path());
        cmd
    }

    pub fn fixture(&self) -> &Path {
        &self.fixture
    }

    pub fn path(&self) -> &Path {
        self.tmp.path()
    }
}

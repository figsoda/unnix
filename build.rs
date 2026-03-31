use std::{env, fs::create_dir_all, path::Path};

use clap::{CommandFactory, ValueEnum};
use clap_complete::Shell;

mod cli {
    include!("src/cli.rs");
}

fn main() {
    println!("cargo:rerun-if-env-changed=GENERATE_ARTIFACTS");

    let Some(dir) = env::var_os("GENERATE_ARTIFACTS") else {
        return;
    };

    let out = Path::new(&dir);
    let cmd = cli::Args::command();
    create_dir_all(out).unwrap();

    clap_mangen::generate_to(cmd.clone(), out).unwrap();

    for shell in Shell::value_variants() {
        clap_complete::generate_to(*shell, &mut cmd.clone(), "unnix", out).unwrap();
    }
}

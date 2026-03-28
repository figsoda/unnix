use std::{
    fs::{File, create_dir_all},
    io::Write,
};

use kdl::{FormatConfig, KdlDocument, KdlNode};
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::{
    cli::{GlobalArgs, InitArgs},
    system::System,
};

pub async fn init(global: GlobalArgs, args: InitArgs) -> Result<()> {
    let dir = if let Some(dir) = global.directory {
        create_dir_all(&dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to create {dir}"))?;
        dir
    } else {
        ".".into()
    };

    let path = dir.join("unnix.kdl");
    let mut file = File::create_new(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create {path}"))?;
    let fmt = FormatConfig::builder().indent("  ").build();

    if !args.systems.is_empty() {
        let mut systems = KdlDocument::new();
        for system in &args.systems {
            system.parse::<System>().into_diagnostic()?;
            systems.nodes_mut().push(KdlNode::new(system.as_str()));
        }

        let mut node = KdlNode::new("systems");
        node.set_children(systems);
        node.autoformat_config(&fmt);
        writeln!(file, "{node}").into_diagnostic()?;
    };

    let mut packages = KdlDocument::new();
    for pkg in &args.packages {
        packages.nodes_mut().push(KdlNode::new(pkg.as_str()));
    }
    let mut node = KdlNode::new("packages");
    node.set_children(packages);
    node.autoformat_config(&fmt);
    write!(file, "{node}").into_diagnostic()?;

    Ok(())
}

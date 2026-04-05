use std::{
    fs::{File, read_to_string},
    io::Write,
};

use kdl::{FormatConfig, KdlDocument, KdlNode};
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::cli::{AddArgs, GlobalArgs};

pub async fn add(global: GlobalArgs, mut args: AddArgs) -> Result<()> {
    let path = global
        .directory
        .unwrap_or_else(|| ".".into())
        .join("unnix.kdl");

    let text = read_to_string(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to open {path}"))?;
    let mut doc = KdlDocument::parse(&text).into_diagnostic()?;

    let node = doc
        .nodes_mut()
        .iter_mut()
        .find(|node| node.name().value() == "packages" && node.entries().is_empty());
    let fmt = FormatConfig::builder().indent("  ").build();

    if let Some(node) = node {
        let leading = node.format().map(|fmt| fmt.leading.clone());
        let nodes = node.ensure_children().nodes_mut();
        let sorted = nodes.is_sorted_by_key(|node| node.name().value());

        nodes.extend(args.packages.into_iter().map(KdlNode::new));
        if sorted {
            nodes.sort_by(|x, y| x.name().value().cmp(y.name().value()));
        }

        node.autoformat_config(&fmt);
        if let Some(leading) = leading
            && let Some(fmt) = node.format_mut()
        {
            fmt.leading = leading;
        }
    } else {
        args.packages.sort();
        let mut packages = KdlDocument::new();
        packages
            .nodes_mut()
            .extend(args.packages.into_iter().map(KdlNode::new));

        let mut node = KdlNode::new("packages");
        node.set_children(packages);
        node.autoformat_config(&fmt);

        let nodes = doc.nodes_mut();
        if !nodes.is_empty()
            && let Some(fmt) = node.format_mut()
        {
            fmt.leading.insert(0, '\n');
        }
        nodes.push(node);
    }

    let mut file = File::options()
        .write(true)
        .open(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to open {path}"))?;
    write!(file, "{doc}").into_diagnostic()?;

    Ok(())
}

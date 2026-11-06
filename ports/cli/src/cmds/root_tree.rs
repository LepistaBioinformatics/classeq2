use anyhow::{anyhow, bail, Result};
use clap::{Parser, ValueEnum};
use classeq_core::domain::rooting::{root_newick, RootingMethod};
use std::{
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum RootingMethodArg {
    /// Root at the midpoint of the longest leaf-to-leaf path
    Midpoint,

    /// Root on the branch separating the --outgroup leaves from the rest
    Outgroup,
}

/// Resolves the rooting method from `--method`/`--root` and `--outgroup`.
///
/// `--outgroup` alone implies the outgroup method.
pub(crate) fn rooting_method(
    method: Option<RootingMethodArg>,
    outgroup: Vec<String>,
) -> Result<Option<RootingMethod>> {
    match (method, outgroup.is_empty()) {
        (None, true) => Ok(None),
        (None, false) | (Some(RootingMethodArg::Outgroup), false) => {
            Ok(Some(RootingMethod::Outgroup(outgroup)))
        }
        (Some(RootingMethodArg::Outgroup), true) => {
            bail!("The outgroup method needs at least one --outgroup leaf name")
        }
        (Some(RootingMethodArg::Midpoint), true) => {
            Ok(Some(RootingMethod::Midpoint))
        }
        (Some(RootingMethodArg::Midpoint), false) => {
            bail!("--outgroup cannot be combined with the midpoint method")
        }
    }
}

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the Newick tree to root, or `-` to read from stdin
    pub(super) input: String,

    /// Output file path
    ///
    /// If not provided, the rooted tree is written to stdout.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Rooting method
    ///
    /// Defaults to midpoint, or to outgroup when --outgroup is given.
    #[arg(long, value_enum)]
    pub(super) method: Option<RootingMethodArg>,

    /// Outgroup leaf names (repeat the flag or separate names with commas)
    #[arg(long, value_delimiter = ',')]
    pub(super) outgroup: Vec<String>,

    /// Write a tree whose root is already binary back without re-rooting it
    /// (unless it is marked `[&U]`)
    #[arg(long)]
    pub(super) keep_if_rooted: bool,
}

pub(crate) fn root_tree_cmd(args: Arguments) -> Result<()> {
    let method = rooting_method(args.method, args.outgroup)?
        .unwrap_or(RootingMethod::Midpoint);

    let content = match args.input.as_str() {
        "-" => {
            let mut content = String::new();
            std::io::stdin().read_to_string(&mut content)?;
            content
        }
        path => std::fs::read_to_string(path)
            .map_err(|err| anyhow!("Could not read file {path}: {err}"))?,
    };

    let rooted = root_newick(&content, &method, args.keep_if_rooted)
        .map_err(|err| anyhow!("{}", err.msg()))?;

    match args.output_file_path {
        Some(path) => std::fs::write(&path, format!("{rooted}\n"))?,
        None => {
            let mut stdout = std::io::stdout().lock();
            writeln!(stdout, "{rooted}")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

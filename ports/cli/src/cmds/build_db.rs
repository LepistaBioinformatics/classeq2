use super::root_tree::{rooting_method, RootingMethodArg};

use anyhow::{anyhow, Result};
use clap::Parser;
use classeq_core::use_cases::map_kmers_to_tree_with_rooting;
use std::{fs::File, path::PathBuf};

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the tree file
    ///
    /// The file should be in Newick format.
    pub(super) tree_file_path: PathBuf,

    /// Path to the msa file
    ///
    /// The file should be in FASTA format.
    pub(super) msa_file_path: PathBuf,

    /// The kmer size
    ///
    /// The size of the kmers to be used in the tree.
    #[arg(long, short, default_value = "35")]
    pub(super) k_size: Option<u64>,

    /// The minimizer size
    ///
    /// The size of the minimizer to be used in the tree.
    #[arg(long, short, default_value = "4")]
    pub(super) m_size: Option<u64>,

    /// Output file path
    ///
    /// If not provided, the output will be saved in the current directory with
    /// the name `classeq-database.cls`.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Minimum branch support
    ///
    /// The minimum branch support value to consider a branch in the tree.
    #[arg(short = 's', long, default_value = "70")]
    pub(super) min_branch_support: Option<f64>,

    /// Root the tree in memory if it is not rooted
    ///
    /// Without this flag an unrooted tree is an error. A tree that is already
    /// rooted is used as is.
    #[arg(long, value_enum)]
    pub(super) root: Option<RootingMethodArg>,

    /// Outgroup leaf names for --root outgroup (repeat the flag or separate
    /// names with commas)
    #[arg(long, value_delimiter = ',')]
    pub(super) outgroup: Vec<String>,
}

pub(crate) fn build_database_cmd(
    args: Arguments,
    threads: Option<usize>,
) -> Result<()> {
    // ? -----------------------------------------------------------------------
    // ? Create a thread pool configured globally
    // ? -----------------------------------------------------------------------

    if let Err(err) = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.unwrap_or(1))
        .build_global()
    {
        panic!("Error creating thread pool: {err}");
    };

    let rooting = rooting_method(args.root, args.outgroup)?;

    let tree = map_kmers_to_tree_with_rooting(
        args.tree_file_path,
        args.msa_file_path,
        args.k_size,
        args.m_size,
        args.min_branch_support,
        rooting,
    )
    .map_err(|err| anyhow!("{}", err.msg()))?;

    let mut output_file_path = args
        .output_file_path
        .unwrap_or_else(|| PathBuf::from("classeq-database.cls"));

    output_file_path.set_extension("cls");

    let writer = File::create(output_file_path)?;
    let mut writer = zstd::Encoder::new(writer, 0)?.auto_finish();
    serde_saphyr::to_io_writer(&mut writer, &tree)?;

    Ok(())
}

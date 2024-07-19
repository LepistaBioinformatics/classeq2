use clap::Parser;
use classeq_core::use_cases::map_kmers_to_tree;
use std::path::PathBuf;

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
    pub(super) k_size: Option<usize>,

    /// The minimizer size
    ///
    /// The size of the minimizer to be used in the tree.
    #[arg(long, short, default_value = "2")]
    pub(super) m_size: Option<usize>,

    /// Output file path
    ///
    /// The file will be saved in JSON or YAML format.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Minimum branch support
    ///
    /// The minimum branch support value to consider a branch in the tree.
    #[arg(long, default_value = "70")]
    pub(super) min_branch_support: Option<f64>,
}

pub(crate) fn build_database_cmd(args: Arguments, threads: Option<usize>) {
    // ? -----------------------------------------------------------------------
    // ? Create a thread pool configured globally
    // ? -----------------------------------------------------------------------

    if let Err(err) = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.unwrap_or(1))
        .build_global()
    {
        panic!("Error creating thread pool: {err}");
    };

    match map_kmers_to_tree(
        args.tree_file_path,
        args.msa_file_path,
        args.k_size,
        args.m_size,
        args.min_branch_support,
    ) {
        Ok(tree) => {
            let content = match serde_yaml::to_string(&tree) {
                Err(err) => {
                    eprintln!("Error: {}", err);
                    return;
                }
                Ok(content) => content,
            };

            match args.output_file_path {
                Some(path) => match std::fs::write(path, content) {
                    Ok(_) => (),
                    Err(err) => eprintln!("Error: {}", err),
                },
                None => println!("{}", content),
            }
        }
        Err(err) => eprintln!("Error: {}", err),
    }
}

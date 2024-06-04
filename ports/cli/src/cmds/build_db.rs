use clap::Parser;
use classeq_core::use_cases::map_kmers_to_tree;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub(crate) struct BuildDatabaseArguments {
    /// Path to the tree file
    ///
    /// The file should be in Newick format.
    pub(super) tree_file_path: PathBuf,

    /// Path to the msa file
    ///
    /// The file should be in FASTA format.
    pub(super) msa_file_path: PathBuf,

    /// Output file path
    ///
    /// The file will be saved in JSON format.
    #[arg(long, short)]
    pub(super) k_size: Option<usize>,

    /// Output file path
    ///
    /// The file will be saved in JSON or YAML format.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Minimum branch support
    ///
    /// The minimum branch support value to consider a branch in the tree.
    #[arg(long)]
    pub(super) min_branch_support: Option<f64>,
}

pub(crate) fn build_database_cmd(args: BuildDatabaseArguments) {
    match map_kmers_to_tree(
        args.tree_file_path,
        args.msa_file_path,
        args.k_size.unwrap_or(31),
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

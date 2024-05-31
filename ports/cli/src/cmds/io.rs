use clap::Parser;
use classeq_core::domain::dtos::{kmers_map::KmersMap, tree::Tree};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    /// JSON format
    Json,

    /// YAML format
    Yaml,
}

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// File conversion related commands
    #[clap(subcommand)]
    pub convert: Commands,
}

#[derive(Parser, Debug)]
pub(crate) enum Commands {
    /// Serialize a tree
    ///
    /// Serialize a tree in Newick format to JSON or YAML formats.
    Tree(SerializeTreeArguments),

    /// Get sequence kmers
    ///
    /// Extract kmers from a sequence.
    Kmers(GetKmersArguments),
}

// ? ---------------------------------------------------------------------------
// ? Serialize a tree
// ? ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
pub(crate) struct SerializeTreeArguments {
    /// Path to the tree file
    ///
    /// The file should be in Newick format.
    pub(super) tree_file_path: PathBuf,

    /// Path to the output file
    ///
    /// If not provided, the output will be printed to the standard output.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Output format
    ///
    /// The format in which the tree will be serialized.
    #[arg(long, default_value = "json")]
    pub(super) out_format: OutputFormat,
}

pub(crate) fn serialize_tree_cmd(args: SerializeTreeArguments) {
    let tree = match Tree::from_file(args.tree_file_path.as_path()) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    let content = match args.out_format {
        OutputFormat::Json => match serde_json::to_string_pretty(&tree) {
            Err(err) => {
                eprintln!("Error: {err}");
                return;
            }
            Ok(content) => content,
        },
        OutputFormat::Yaml => match serde_yaml::to_string(&tree) {
            Err(err) => {
                eprintln!("Error: {err}");
                return;
            }
            Ok(content) => content,
        },
    };

    match args.output_file_path {
        Some(path) => {
            if let Err(err) = std::fs::write(path.as_path(), content) {
                eprintln!("Error: {err}")
            }
        }
        None => println!("{}", content),
    }
}

// ? ---------------------------------------------------------------------------
// ? Get sequence kmers
// ? ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
pub(crate) struct GetKmersArguments {
    /// Path to the MSA file
    ///
    /// The file should be in FASTA format.
    pub(super) sequence: String,

    /// Kmer length
    ///
    /// The length of the kmers to be extracted.
    #[arg(short, long, default_value = "31")]
    pub(super) kmer_length: usize,
}

pub(crate) fn get_kmers_cmd(args: GetKmersArguments) {
    for kmer in
        KmersMap::build_kmers_from_string(args.sequence, args.kmer_length)
    {
        println!("{}", kmer);
    }
}

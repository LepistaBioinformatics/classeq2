use crate::dtos::output_format::OutputFormat as CliOutputFormat;

use anyhow::Result;
use clap::Parser;
use classeq_core::domain::dtos::{
    kmers_map::KmersMap, output_format::OutputFormat, tree::Tree,
};
use classeq_ports_lib::load_database;
use std::{fs::File, path::PathBuf};

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

    /// Convert Classeq database
    ///
    /// Convert a Classeq database between yaml and binary formats. This is util
    Database(DatabaseArguments),
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

    /// Minimum branch support
    ///
    /// The minimum branch support value to consider a branch in the tree.
    #[arg(long)]
    pub(super) min_branch_support: Option<f64>,

    /// Output format
    ///
    /// The format in which the tree will be serialized.
    #[arg(long, default_value = "yaml")]
    pub(super) out_format: OutputFormat,
}

pub(crate) fn serialize_tree_cmd(args: SerializeTreeArguments) {
    let tree = match Tree::init_from_file(
        args.tree_file_path.as_path(),
        args.min_branch_support.unwrap_or(95.0),
    ) {
        Ok(tree) => tree,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    let content = match args.out_format {
        OutputFormat::Jsonl => match serde_json::to_string_pretty(&tree) {
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
    pub(super) kmer_length: u64,
}

pub(crate) fn get_kmers_cmd(args: GetKmersArguments) {
    let mapper = KmersMap::new(args.kmer_length, 0);
    for (kmer, _) in mapper.build_kmer_from_string(args.sequence, None) {
        println!("{}", kmer);
    }
}

// ? ---------------------------------------------------------------------------
// ? Database
// ? ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
pub(crate) struct DatabaseArguments {
    /// Path to the database file
    ///
    /// The file should be in Classeq database in YAML or Binary format.
    #[arg(short, long)]
    pub(super) database_file_path: PathBuf,

    /// Path to the output file
    ///
    /// If not provided, the output will be printed to the standard output.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Output format
    ///
    /// The format in which the database will be serialized.
    #[arg(long, short = 'f', default_value = "yaml")]
    pub(super) out_format: CliOutputFormat,
}

pub(crate) fn convert_database_cmd(args: DatabaseArguments) -> Result<()> {
    let tree_content = load_database(args.database_file_path)?;
    let mut output_file_path = args
        .output_file_path
        .unwrap_or_else(|| PathBuf::from("classeq-database"));

    //
    // Serialize the content
    //
    match args.out_format {
        CliOutputFormat::Zstd => {
            output_file_path.set_extension("cls");
            let writer = File::create(output_file_path)?;
            let writer = zstd::Encoder::new(writer, 0)?.auto_finish();
            serde_yaml::to_writer(writer, &tree_content)?;
        }
        CliOutputFormat::Yaml => {
            output_file_path.set_extension("cls.yaml");
            let writer = File::create(output_file_path)?;
            serde_yaml::to_writer(writer, &tree_content)?;
        }
    };

    Ok(())
}

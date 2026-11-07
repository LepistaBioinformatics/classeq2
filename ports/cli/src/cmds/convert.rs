use crate::dtos::output_format::DatabaseOutputFormat;

use anyhow::{anyhow, Result};
use clap::{ArgAction, Parser};
use classeq_core::domain::dtos::{
    kmers_map::KmersMap, output_format::OutputFormat, tree::Tree,
};
use classeq_ports_lib::load_database;
use std::{fs::File, io::BufWriter, path::PathBuf};

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
    /// Convert a Classeq database between the binary, zstd, yaml and json
    /// formats.
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

pub(crate) fn serialize_tree_cmd(args: SerializeTreeArguments) -> Result<()> {
    let tree = match Tree::init_from_file(
        args.tree_file_path.as_path(),
        args.min_branch_support.unwrap_or(95.0),
    ) {
        Ok(tree) => tree,
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    };

    let content = match args.out_format {
        OutputFormat::Jsonl => serde_json::to_string_pretty(&tree)?,
        OutputFormat::Yaml => serde_saphyr::to_string(&tree)?,
    };

    match args.output_file_path {
        Some(path) => std::fs::write(path.as_path(), content)?,
        None => println!("{}", content),
    }

    Ok(())
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

    /// Only tree
    ///
    /// If true, only the tree will be extracted from the database.
    #[arg(long, short = 't', action=ArgAction::SetTrue, default_value = "false")]
    pub(super) only_tree: Option<bool>,

    /// Path to the output file
    ///
    /// If not provided, the output will be printed to the standard output.
    #[arg(short, long)]
    pub(super) output_file_path: Option<PathBuf>,

    /// Output format
    ///
    /// The format in which the database will be serialized.
    #[arg(long, short = 'f', default_value = "yaml")]
    pub(super) out_format: DatabaseOutputFormat,
}

pub(crate) fn convert_database_cmd(args: DatabaseArguments) -> Result<()> {
    let mut tree_content = load_database(args.database_file_path)?;
    let only_tree = args.only_tree.unwrap_or(false);

    //
    // The text formats carry the k-mers as a `KmersMap`: rebuild it from the
    // index the loader froze.
    //
    if !only_tree && !matches!(args.out_format, DatabaseOutputFormat::Binary) {
        if let Some(index) = tree_content.index.take() {
            tree_content.kmers_map = Some(index.thaw()?);
        }
    }

    let mut output_file_path = args
        .output_file_path
        .unwrap_or_else(|| PathBuf::from("classeq-database"));

    //
    // Serialize the content
    //
    match args.out_format {
        DatabaseOutputFormat::Zstd => {
            output_file_path.set_extension("cls");
            let writer = File::create(output_file_path)?;
            let mut writer = zstd::Encoder::new(writer, 0)?.auto_finish();

            if only_tree {
                serde_saphyr::to_io_writer(&mut writer, &tree_content.root)?;
            } else {
                serde_saphyr::to_io_writer(&mut writer, &tree_content)?;
            };
        }
        DatabaseOutputFormat::Yaml => {
            output_file_path.set_extension("cls.yaml");
            let mut writer = File::create(output_file_path)?;

            if only_tree {
                serde_saphyr::to_io_writer(&mut writer, &tree_content.root)?;
            } else {
                serde_saphyr::to_io_writer(&mut writer, &tree_content)?;
            };
        }
        DatabaseOutputFormat::Binary => {
            if only_tree {
                return Err(anyhow!(
                    "--only-tree is not supported by the binary format"
                ));
            }

            let index = tree_content
                .index
                .as_ref()
                .ok_or_else(|| anyhow!("The database does not have k-mers."))?;

            output_file_path.set_extension("cls");
            index.write(
                &tree_content,
                BufWriter::new(File::create(output_file_path)?),
            )?;
        }
        DatabaseOutputFormat::Json => {
            output_file_path.set_extension("cls.json");
            let writer = File::create(output_file_path)?;

            if only_tree {
                serde_json::to_writer_pretty(writer, &tree_content.root)?;
            } else {
                serde_json::to_writer_pretty(writer, &tree_content)?;
            };
        }
    };

    Ok(())
}

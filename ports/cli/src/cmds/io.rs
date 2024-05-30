use clap::Parser;
use classeq_core::domain::dtos::tree::Tree;
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
}

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

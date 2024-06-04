use clap::Parser;
use classeq_core::{
    domain::dtos::{
        file_or_stdin::FileOrStdin, output_format::OutputFormat, tree::Tree,
    },
    use_cases::place_sequences,
};
use std::{fs::read_to_string, path::PathBuf};

#[derive(Parser, Debug)]
pub(crate) struct PlaceSequencesArguments {
    /// If the value is "-", the STDIN will be used and this command will expect
    /// to receive the blutils output from the STDIN.
    #[clap(default_value = "-")]
    pub(super) query: FileOrStdin,

    /// Path to the classeq database
    ///
    /// The file should be in JSON or YAML format.
    #[arg(short, long)]
    pub(super) database_file_path: PathBuf,

    /// Output file path
    ///
    /// The file will be saved in JSON or YAML format.
    #[arg(short, long)]
    pub(super) output_file_path: PathBuf,

    /// Output format
    ///
    /// The format in which the tree will be serialized.
    #[arg(long, default_value = "yaml")]
    pub(super) out_format: OutputFormat,

    /// Maximum number of iterations
    ///
    /// The maximum number of iterations to traverse the tree.
    #[arg(short, long)]
    pub(super) max_iterations: Option<i32>,

    /// Force overwrite
    ///
    /// If the output file already exists, it will be overwritten.
    #[arg(short, long, default_value = "false")]
    pub(super) force_overwrite: bool,
}

pub(crate) fn place_sequences_cmd(args: PlaceSequencesArguments) {
    let database_file = match read_to_string(&args.database_file_path) {
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
        Ok(content) => content,
    };

    let tree = match serde_yaml::from_str::<Tree>(database_file.as_str()) {
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
        Ok(buffer) => buffer,
    };

    if let Err(err) = place_sequences(
        args.query,
        tree,
        args.output_file_path,
        args.max_iterations,
        &args.force_overwrite,
        args.out_format,
    ) {
        panic!("{err}")
    };
}

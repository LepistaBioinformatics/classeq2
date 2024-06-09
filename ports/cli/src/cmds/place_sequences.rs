use clap::Parser;
use classeq_core::{
    domain::dtos::{
        file_or_stdin::FileOrStdin, output_format::OutputFormat, tree::Tree,
    },
    use_cases::place_sequences,
};
use std::{fs::read_to_string, path::PathBuf};
use tracing::info;

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
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

pub(crate) fn place_sequences_cmd(args: Arguments, threads: usize) {
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

    let times = place_sequences(
        args.query,
        tree,
        args.output_file_path,
        args.max_iterations,
        &args.force_overwrite,
        args.out_format,
        threads,
    );

    let times = times
        .into_iter()
        .map(|time| time.time as f64)
        .collect::<Vec<_>>();

    let average_time: f64 = times.iter().sum::<f64>() / times.len() as f64;
    let max_time: f64 = *times
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    let min_time: f64 = *times
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    let total_in_seconds: f64 = times.iter().sum();

    info!("Placement times:");
    info!("total\taverage\tmax\tmin");
    info!(
        "{:.2}\t{:.2}\t{:.2}\t{:.2}",
        total_in_seconds, average_time, max_time, min_time
    );
}

use crate::dtos::telemetry_code::TelemetryCode;

use clap::{ArgAction, Parser};
use classeq_core::{
    domain::dtos::{
        file_or_stdin::FileOrStdin, output_format::OutputFormat, tree::Tree,
    },
    use_cases::place_sequences,
};
use std::time::Instant;
use std::{fs::read_to_string, path::PathBuf, time::Duration};
use tracing::{info, info_span};
use uuid::Uuid;

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
    pub(super) iterations: Option<i32>,

    /// Minimum match coverage
    ///
    /// The minimum match coverage between the query and the database sequences.
    #[arg(short, long)]
    pub(super) match_coverage: Option<f64>,

    /// Remove intersection
    ///
    /// If true, calculate the one-vs-rest difference without the shared kmers.
    /// Otherwise, calculate the one-vs-rest difference with the shared kmers.
    #[arg(short, long, action=ArgAction::SetTrue)]
    pub(super) remove_intersection: Option<bool>,

    /// Force overwrite
    ///
    /// If the output file already exists, it will be overwritten.
    #[arg(short, long, default_value = "false")]
    pub(super) force_overwrite: bool,
}

pub(crate) fn place_sequences_cmd(args: Arguments, threads: usize) {
    let span = info_span!(
        "PlacingSequenceCMD",
        run_id = Uuid::new_v4().to_string().replace("-", "")
    );

    let _span_guard = span.enter();

    info!(
        code = TelemetryCode::CLIPLACE0001.to_string(),
        "Start multiple sequences placement from CLI"
    );

    // ? -----------------------------------------------------------------------
    // ? Create a thread pool configured globally
    // ? -----------------------------------------------------------------------

    if let Err(err) = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.to_owned())
        .build_global()
    {
        panic!("Error creating thread pool: {err}");
    };

    let now = Instant::now();

    let per_seq_time = {
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

        match place_sequences(
            args.query,
            &tree,
            &args.output_file_path,
            &args.iterations,
            &args.match_coverage,
            &args.force_overwrite,
            &args.out_format,
            &args.remove_intersection,
        ) {
            Ok(buffer) => buffer,
            Err(err) => panic!("{err}"),
        }
    };

    let elapsed = now.elapsed();

    let average = per_seq_time
        .to_owned()
        .into_iter()
        .map(|i| i.milliseconds_time)
        .sum::<Duration>()
        / per_seq_time.len() as u32;

    let max = per_seq_time
        .to_owned()
        .into_iter()
        .map(|i| i.milliseconds_time)
        .max()
        .unwrap_or_default();

    let min = per_seq_time
        .to_owned()
        .into_iter()
        .map(|i| i.milliseconds_time)
        .min()
        .unwrap_or_default();

    info!(
        code = TelemetryCode::CLIPLACE0002.to_string(),
        totalSeconds = elapsed.as_secs_f32(),
        averageSeconds = average.as_secs_f32(),
        maxSeconds = max.as_secs_f32(),
        minSeconds = min.as_secs_f32(),
        "Execution times"
    );
}

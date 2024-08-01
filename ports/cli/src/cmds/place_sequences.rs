use crate::dtos::telemetry_code::TelemetryCode;

use anyhow::Result;
use clap::{ArgAction, Parser};
use classeq_core::{
    domain::dtos::{
        annotation::Annotation, file_or_stdin::FileOrStdin,
        output_format::OutputFormat,
    },
    use_cases::place_sequences,
};
use classeq_ports_lib::load_database;
use std::time::Instant;
use std::{path::PathBuf, time::Duration};
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

    /// Path to the annotations file
    ///
    /// The filepath to the annotations in YAML format.
    #[arg(short, long)]
    pub(super) annotations_file_path: Option<PathBuf>,

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

    /// Generate profiling
    ///
    /// If true, generate a classeq-profile.pb file used to profile the
    /// placement process. The resulting file should ve visualized using the
    /// pprof tool (https://pkg.go.dev/github.com/google/pprof#section-readme).
    #[cfg(feature = "profiling")]
    #[arg(short = 'p', long, default_value = "false")]
    pub(super) with_profiling: bool,
}

pub(crate) fn place_sequences_cmd(
    args: Arguments,
    threads: usize,
) -> Result<()> {
    // ? -----------------------------------------------------------------------
    // ? Configure profiling
    // ? -----------------------------------------------------------------------

    #[cfg(feature = "profiling")]
    let profiling_guard: Option<pprof::ProfilerGuard> = if args.with_profiling {
        Some(
            pprof::ProfilerGuardBuilder::default()
                .frequency(1000)
                .blocklist(&["libc", "libgcc", "pthread", "vdso"])
                .build()
                .unwrap(),
        )
    } else {
        None
    };

    // ? -----------------------------------------------------------------------
    // ? Configure logging
    // ? -----------------------------------------------------------------------

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
        let mut tree = load_database(args.database_file_path)?;

        if let Some(path) = args.annotations_file_path {
            let content: Vec<Annotation> =
                serde_yaml::from_reader(std::fs::File::open(path)?)?;

            if !content.is_empty() {
                tree.annotations = Some(content);
            }
        }

        match place_sequences(
            args.query,
            &tree,
            &args.output_file_path,
            &args.iterations,
            &args.match_coverage,
            &args.force_overwrite,
            &args.out_format,
            &args.remove_intersection,
            &Some(&span),
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

    // ? -----------------------------------------------------------------------
    // ? Run profiling
    // ? -----------------------------------------------------------------------

    #[cfg(feature = "profiling")]
    if let Some(guard) = profiling_guard {
        use pprof::protos::Message;
        use std::{fs::File, io::Write};

        let mut path = (match args.output_file_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => PathBuf::new(),
        })
        .join("classeq-profile");
        path.set_extension("pb");
        let mut file = File::create(path)?;

        let report = guard.report().build()?;
        let profile = report.pprof()?;
        let mut content = Vec::new();
        profile.encode(&mut content).unwrap();
        file.write_all(&content).unwrap();
    }

    // ? -----------------------------------------------------------------------
    // ? Return a positive response
    // ? -----------------------------------------------------------------------

    Ok(())
}

mod cmds;

use self::Opts::*;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};
use tracing::{debug, warn};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
enum LogFormat {
    /// ANSI format
    ///
    /// This format is human-readable and colorful.
    Ansi,

    /// YAML format
    ///
    /// This format is machine-readable and can be used for log analysis.
    Json,
}

#[derive(Subcommand, Debug)]
#[command(author, version, about, long_about = None)]
enum Opts {
    /// Input/Output commands
    Convert(cmds::convert::Arguments),

    /// Build the index database
    BuildDb(cmds::build_db::Arguments),

    /// Place sequences on the tree
    Place(cmds::place_sequences::Arguments),
}

#[derive(Parser, Debug)]
#[clap(name = "cls", version, author, about)]
struct Cli {
    #[clap(subcommand)]
    opts: Opts,

    #[clap(long)]
    log_level: Option<String>,

    #[clap(long)]
    log_file: Option<String>,

    #[clap(long, default_value = "ansi")]
    log_format: LogFormat,

    #[clap(short, long, default_value = "1")]
    threads: Option<usize>,
}

/// Get the command line arguments.
#[tracing::instrument(name = "Runtime arguments")]
fn get_arguments() {
    let args: Vec<_> = std::env::args().collect();
    debug!("{:?}", args.join(" "));
}

fn main() {
    let args = Cli::parse();

    // ? -----------------------------------------------------------------------
    // ? Configure logger
    // ? -----------------------------------------------------------------------

    let log_level = args.log_level.unwrap_or("info".to_string());

    let (non_blocking, _guard) = match args.log_file {
        //
        // If no log file is provided, log to stderr
        //
        None => tracing_appender::non_blocking(std::io::stderr()),
        //
        // If a log file is provided, log to the file
        //
        Some(file) => {
            let log_file = PathBuf::from(file);

            let file_appender = tracing_appender::rolling::minutely(
                log_file.parent().unwrap(),
                log_file.file_name().unwrap(),
            );

            tracing_appender::non_blocking(file_appender)
        }
    };

    let tracing_config = tracing_subscriber::fmt()
        .event_format(
            fmt::format()
                // don't include levels in formatted output
                .with_level(true)
                // don't include targets
                .with_target(false)
                .compact(),
        )
        .with_writer(non_blocking)
        .with_env_filter(EnvFilter::from_str(log_level.as_str()).unwrap());

    match args.log_format {
        LogFormat::Ansi => tracing_config.pretty().init(),
        LogFormat::Json => tracing_config.json().init(),
    };

    // ? -----------------------------------------------------------------------
    // ? Get command line arguments
    // ? -----------------------------------------------------------------------

    get_arguments();

    match args.opts {
        Convert(io_args) => match io_args.convert {
            cmds::convert::Commands::Tree(tree_args) => {
                cmds::convert::serialize_tree_cmd(tree_args);
            }
            cmds::convert::Commands::Kmers(kmers_args) => {
                cmds::convert::get_kmers_cmd(kmers_args);
            }
        },
        BuildDb(db_args) => cmds::build_db::build_database_cmd(db_args),
        Place(place_args) => cmds::place_sequences::place_sequences_cmd(
            place_args,
            args.threads.unwrap(),
        ),
    }
}

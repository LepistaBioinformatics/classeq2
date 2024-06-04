mod cmds;

use self::Cli::*;

use clap::Parser;
use std::str::FromStr;
use tracing::debug;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Cli {
    /// Input/Output commands
    Convert(cmds::convert::Arguments),

    /// Build the index database
    BuildDb(cmds::build_db::BuildDatabaseArguments),

    /// Place sequences on the tree
    Place(cmds::place_sequences::PlaceSequencesArguments),
}

/// Get the command line arguments.
fn get_arguments() {
    let args: Vec<_> = std::env::args().collect();
    debug!("{:?}", args.join(" "));
}

fn main() {
    let log_level = std::env::var("RUST_LOG").unwrap_or("error".to_string());

    let tracing_config = tracing_subscriber::fmt()
        .event_format(
            fmt::format()
                // don't include levels in formatted output
                .with_level(true)
                // don't include targets
                .with_target(false)
                .compact(),
        )
        .with_env_filter(EnvFilter::from_str(log_level.as_str()).unwrap());

    if std::env::var("RUST_LOG_FORMAT").unwrap_or("".to_string()) == "json" {
        tracing_config.json().init();
    } else {
        tracing_config.with_ansi(true).init();
    }

    get_arguments();

    match Cli::parse() {
        Convert(io_args) => match io_args.convert {
            cmds::convert::Commands::Tree(tree_args) => {
                cmds::convert::serialize_tree_cmd(tree_args)
            }
            cmds::convert::Commands::Kmers(kmers_args) => {
                cmds::convert::get_kmers_cmd(kmers_args);
            }
        },
        BuildDb(db_args) => cmds::build_db::build_database_cmd(db_args),
        Place(place_args) => {
            cmds::place_sequences::place_sequences_cmd(place_args)
        }
    }
}

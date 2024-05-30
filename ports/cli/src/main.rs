mod cmds;

use self::Cli::*;

use clap::Parser;
use cmds::io;
use std::str::FromStr;
use tracing::debug;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
enum Cli {
    /// Input/Output commands
    Convert(io::Arguments),
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
            io::Commands::Tree(tree_args) => io::serialize_tree_cmd(tree_args),
        },
    }
}

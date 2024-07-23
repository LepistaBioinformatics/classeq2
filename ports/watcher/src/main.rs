mod cmds;
mod models;

use anyhow::Result;
use clap::Subcommand;
use classeq_ports_lib::{expose_runtime_arguments, CliLauncher, LogFormat};
use std::{path::PathBuf, str::FromStr};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Subcommand, Debug)]
#[command(author, version, about, long_about = None)]
enum Opts {
    Watch(cmds::watch_dir::Arguments),
}

#[async_std::main]
async fn main() -> Result<()> {
    let args = CliLauncher::<Opts>::parse();

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
        .event_format(fmt::format().with_level(true).compact())
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .with_writer(non_blocking)
        .with_env_filter(EnvFilter::from_str(log_level.as_str()).unwrap());

    match args.log_format {
        LogFormat::Ansi => tracing_config.pretty().init(),
        LogFormat::Jsonl => tracing_config.json().init(),
    };

    // ? -----------------------------------------------------------------------
    // ? Get command line arguments
    // ? -----------------------------------------------------------------------

    expose_runtime_arguments();

    // ? -----------------------------------------------------------------------
    // ? Configure the runtime
    // ? -----------------------------------------------------------------------

    match args.opts {
        Opts::Watch(watch_args) => {
            cmds::watch_dir::start_watch_directory_cmd(watch_args).await?
        }
    };

    Ok(())
}

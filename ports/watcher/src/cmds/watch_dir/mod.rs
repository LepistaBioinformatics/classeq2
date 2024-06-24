mod context;

use crate::models::{
    config_file::ConfigFile,
    reminder::{Reminder, ReminderSpan},
};

use anyhow::Result;
use apalis::{
    cron::{CronStream, Schedule},
    layers::{
        retry::{RetryLayer, RetryPolicy},
        tracing::TraceLayer,
    },
    prelude::Data,
    utils::AsyncStdExecutor,
};
use apalis_core::{
    builder::{WorkerBuilder, WorkerFactoryFn},
    monitor::Monitor,
};
use clap::Parser;
use classeq_ports_lib::FileSystemConfig;
use context::WorkerCtx;
use std::{path::PathBuf, str::FromStr};
use tracing::{debug, info, Instrument};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the configuration file
    ///
    /// Configuration file in YAML format.
    #[arg(short, long)]
    pub(super) config_file: PathBuf,
}

pub(crate) async fn start_watch_directory_cmd(
    args: Arguments,
    _: usize,
) -> Result<()> {
    // ? -----------------------------------------------------------------------
    // ? Setup the Ctrl-C handler
    // ? -----------------------------------------------------------------------

    let (s, ctrl_c) = async_channel::bounded(1);

    let handle = move || {
        s.try_send(()).ok();
    };

    ctrlc::set_handler(handle)?;

    // ? -----------------------------------------------------------------------
    // ? Setup the worker
    // ? -----------------------------------------------------------------------

    let config = ConfigFile::from_file(&args.config_file)?;

    let schedule = Schedule::from_str(
        format!("1/{seconds} * * * * *", seconds = config.watcher.interval)
            .as_str(),
    )
    .unwrap();

    let worker = WorkerBuilder::new(config.watcher.worker_name)
        .layer(RetryLayer::new(RetryPolicy::retries(
            config.watcher.retries as usize,
        )))
        .layer(TraceLayer::new().make_span_with(ReminderSpan::new()))
        .data(config.fs)
        .stream(CronStream::new(schedule).into_stream())
        .build_fn(dispatch_scan);

    // ? -----------------------------------------------------------------------
    // ? Run the worker
    // ? -----------------------------------------------------------------------

    Monitor::<AsyncStdExecutor>::new()
        .register_with_count(1, worker)
        .on_event(|e| debug!("Worker event: {e:?}"))
        .run_with_signal(async {
            ctrl_c.recv().await.ok();
            info!("Shutting down");
            Ok(())
        })
        .await?;

    Ok(())
}

/// Dispatches the scan task to the worker
///
/// This function dispatches the scan task to the worker, allowing the scan to
/// be executed in the background.
///
async fn dispatch_scan(
    _: Reminder,
    worker: WorkerCtx,
    data: Data<FileSystemConfig>,
) -> bool {
    worker.spawn(scan_directories_in_background(data).in_current_span());
    true
}

/// Scans the directories in the background
///
/// This function scans the directories in the background.
///
async fn scan_directories_in_background(fs_config: Data<FileSystemConfig>) {
    //
    // Scan public directory
    //
    let directory_content: Vec<PathBuf> = WalkDir::new(
        &PathBuf::from(&fs_config.serve_directory)
            .join(fs_config.public_directory.to_owned()),
    )
    .contents_first(true)
    .sort_by_file_name()
    .min_depth(1)
    .max_depth(1)
    .into_iter()
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.path().to_path_buf())
    .filter_map(|path| {
        let config_file = path.join(fs_config.config_file_name.to_owned());

        if config_file.exists() &&
            !path.join(fs_config.success_file_name.to_owned()).exists() &&
            !path.join(fs_config.running_file_name.to_owned()).exists() &&
            !path.join(fs_config.error_file_name.to_owned()).exists()
        {
            Some(config_file)
        } else {
            None
        }
    })
    .collect();

    println!("directory_content: {:?}", directory_content);
}

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

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the configuration file
    ///
    /// Configuration file in YAML format.
    #[arg(short, long)]
    pub(super) config_file: PathBuf,
}

async fn send_in_background(data: Data<FileSystemConfig>) {
    //apalis_core::sleep(Duration::from_secs(2)).await;
    println!("data: {:?}", data);
}

async fn send_reminder(
    _: Reminder,
    worker: WorkerCtx,
    data: Data<FileSystemConfig>,
) -> bool {
    // this will happen in the workers background and wont block the next tasks
    worker.spawn(send_in_background(data).in_current_span());
    true
}

pub(crate) async fn start_watch_directory_cmd(
    args: Arguments,
    _: usize,
) -> Result<()> {
    let (s, ctrl_c) = async_channel::bounded(1);
    let handle = move || {
        s.try_send(()).ok();
    };

    let config = ConfigFile::from_file(&args.config_file)?;

    ctrlc::set_handler(handle)?;

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
        .build_fn(send_reminder);

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

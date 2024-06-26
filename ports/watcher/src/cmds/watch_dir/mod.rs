mod context;

use crate::models::{
    config_file::ConfigFile,
    execution_msg::ExecutionMsg,
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
use classeq_ports_lib::{BluAnalysisConfig, FileSystemConfig, ModelsConfig};
use context::WorkerCtx;
use std::{path::PathBuf, str::FromStr};
use tracing::{debug, info, warn, Instrument};

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

    ctrlc::set_handler(move || {
        s.try_send(()).ok();
    })?;

    // ? -----------------------------------------------------------------------
    // ? Setup the dir-watcher worker
    // ? -----------------------------------------------------------------------

    let config = ConfigFile::from_file(&args.config_file)?;

    let schedule = match Schedule::from_str(
        format!("1/{seconds} * * * * *", seconds = config.watcher.interval)
            .as_str(),
    ) {
        Ok(schedule) => schedule,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to parse the schedule: {e}"));
        }
    };

    let worker = WorkerBuilder::new(config.watcher.worker_name)
        .layer(RetryLayer::new(RetryPolicy::retries(
            config.watcher.retries as usize,
        )))
        .layer(TraceLayer::new().make_span_with(ReminderSpan::new()))
        .data(config.fs)
        .data(config.models)
        .stream(CronStream::new(schedule).into_stream())
        .build_fn(dispatch_scan);

    // ? -----------------------------------------------------------------------
    // ? Run the worker
    // ? -----------------------------------------------------------------------

    Monitor::<AsyncStdExecutor>::new()
        .register_with_count(config.watcher.workers as usize, worker)
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
    fs_data: Data<FileSystemConfig>,
    models_data: Data<ModelsConfig>,
) -> bool {
    worker.spawn(
        scan_directories_in_background(fs_data, models_data).in_current_span(),
    );
    true
}

///
/// TODO: do finish implementation
///
/// Scans the directories in the background
///
/// This function scans the directories in the background.
///
async fn scan_directories_in_background(
    fs_config: Data<FileSystemConfig>,
    models_data: Data<ModelsConfig>,
) {
    //
    // Scan public directory
    //
    for path in PathBuf::from(&fs_config.serve_directory)
        .join(fs_config.public_directory.to_owned())
        .read_dir()
        .into_iter()
        .flat_map(|entry| entry)
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
        .into_iter()
    {
        let config_content = match serde_yaml::from_str::<BluAnalysisConfig>(
            &std::fs::read_to_string(&path).unwrap(),
        ) {
            Ok(config_content) => config_content,
            Err(e) => {
                warn!("Failed to parse the configuration file: {e}");
                continue;
            }
        };

        println!("{:?}", config_content);

        let target_model = if let Some(model) = models_data
            .get_models()
            .into_iter()
            .find(|model| model.id == config_content.model_id)
        {
            model
        } else {
            if let Err(err) = std::fs::write(
                path.join(fs_config.error_file_name.to_owned()),
                match serde_yaml::to_string(&ExecutionMsg {
                    msg: format!(
                        "Model with ID {id} not found",
                        id = config_content.model_id
                    ),
                }) {
                    Ok(content) => content,
                    Err(e) => {
                        warn!("Failed to serialize the error message: {e}");
                        continue;
                    }
                },
            ) {
                warn!("Failed to write the error file: {err}");
            };

            continue;
        };

        println!("{:?}", target_model);
    }
}

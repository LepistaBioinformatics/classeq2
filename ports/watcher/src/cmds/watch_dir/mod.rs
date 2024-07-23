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
use async_std::task::sleep;
use clap::Parser;
use classeq_core::{
    domain::dtos::{file_or_stdin::FileOrStdin, tree::Tree},
    use_cases::place_sequences,
};
use classeq_ports_lib::{
    get_file_by_inode, BluAnalysisConfig, FileSystemConfig, ModelsConfig,
};
use context::WorkerCtx;
use rand::{thread_rng, Rng};
use std::{path::PathBuf, str::FromStr, time::Duration};
use tracing::{debug, error, info, warn, Instrument};

#[derive(Parser, Debug)]
pub(crate) struct Arguments {
    /// Path to the configuration file
    ///
    /// Configuration file in YAML format.
    #[arg(short, long)]
    pub(super) config_file: PathBuf,
}

pub(crate) async fn start_watch_directory_cmd(args: Arguments) -> Result<()> {
    // ? -----------------------------------------------------------------------
    // ? Setup the Ctrl-C handler
    // ? -----------------------------------------------------------------------

    let (s, ctrl_c) = async_channel::bounded(1);

    ctrlc::set_handler(move || {
        s.try_send(()).ok();
    })?;

    // ? -----------------------------------------------------------------------
    // ? Load the configuration file
    // ? -----------------------------------------------------------------------

    let config = ConfigFile::from_file(&args.config_file)?;

    // ? -----------------------------------------------------------------------
    // ? Create a thread pool configured globally
    // ? -----------------------------------------------------------------------

    if let Err(err) = rayon::ThreadPoolBuilder::new()
        .num_threads(config.watcher.max_threads.to_owned() as usize)
        .build_global()
    {
        error!("Error creating thread pool: {err}");
    };

    // ? -----------------------------------------------------------------------
    // ? Setup the dir-watcher worker
    // ? -----------------------------------------------------------------------

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
        .data(config.watcher.interval as i32)
        .stream(CronStream::new(schedule).into_stream())
        .build_fn(scan_dispatcher);

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
async fn scan_dispatcher(
    _: Reminder,
    worker: WorkerCtx,
    fs_data: Data<FileSystemConfig>,
    models_data: Data<ModelsConfig>,
    interval: Data<i32>,
) -> bool {
    let max_delay = interval.to_owned().abs();
    let rand_delay = thread_rng().gen_range(1..=max_delay);
    sleep(Duration::from_secs(rand_delay as u64)).await;

    worker.spawn(
        scan_directories_in_background(fs_data, models_data).in_current_span(),
    );

    true
}

/// Scans the directories and dispatches the tasks
///
/// This function scans the directories and dispatches the tasks to the worker
/// for processing.
///
async fn scan_directories_in_background(
    fs_config: Data<FileSystemConfig>,
    models_data: Data<ModelsConfig>,
) {
    //
    // Scan public directory
    //
    // Here only the public directories are scanned. The public directories are
    // directories that contain the analysis configuration files, but not
    // include the success, running, and error files, indicating pending
    // analysis.
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

            if config_file.exists()
                && !path.join(fs_config.success_file_name.to_owned()).exists()
                && !path.join(fs_config.running_file_name.to_owned()).exists()
                && !path.join(fs_config.error_file_name.to_owned()).exists()
            {
                Some(config_file)
            } else {
                None
            }
        })
        .into_iter()
    {
        // ? -------------------------------------------------------------------
        // ? Load the analysis configuration file
        //
        // Directories returned during the scan are expected to contain an
        // analysis configuration file. Analysis configuration file contain the
        // model ID and the query file ID to perform the analysis.
        //
        // ? -------------------------------------------------------------------

        let blutils_config = match BluAnalysisConfig::from_yaml_file(&path) {
            Ok(config_content) => config_content,
            Err(e) => {
                warn!("Failed to parse the configuration file: {e}");
                continue;
            }
        };

        // ? -------------------------------------------------------------------
        // ? Load the target model
        //
        // The target model confain information from the model to be used during
        // prediciotns.
        //
        // ? -------------------------------------------------------------------

        let analysis_model = if let Some(model) = models_data
            .get_models()
            .into_iter()
            .find(|model| model.id == blutils_config.model_id)
        {
            model
        } else {
            if let Err(err) = ExecutionMsg::write_file(
                &path.join(fs_config.error_file_name.to_owned()),
                format!(
                    "Model with ID {id} not found",
                    id = blutils_config.model_id
                )
                .as_str(),
            ) {
                warn!("Failed to write the error file: {err}");
            };

            continue;
        };

        // ? -------------------------------------------------------------------
        // ? Load the model artifacts
        //
        // The model artifact is a Tree struct containning the model used for
        // predictions.
        //
        // ? -------------------------------------------------------------------

        let tree_model = Tree::from_yaml_file(&analysis_model.get_path())
            .expect("Failed to read the tree file");

        // ? -------------------------------------------------------------------
        // ? Load the Query file
        //
        // The query file is a file containing the sequences to be processed.
        //
        // ? -------------------------------------------------------------------

        let (query_file_path, parent) = match path.parent() {
            Some(parent) => {
                let inode_file = get_file_by_inode(
                    parent.join(fs_config.input_directory.to_owned()),
                    blutils_config.query_file_id,
                );

                match inode_file {
                    Some(file) => (file, parent),
                    None => {
                        if let Err(err) = ExecutionMsg::write_file(
                            &parent
                                .to_owned()
                                .join(fs_config.error_file_name.to_owned()),
                            format!(
                                "Query file with inode {inode} not found",
                                inode = blutils_config.query_file_id
                            )
                            .as_str(),
                        ) {
                            panic!("Failed to write the error file: {err}");
                        };

                        continue;
                    }
                }
            }
            None => {
                if let Err(err) = ExecutionMsg::write_file(
                    &path
                        .parent()
                        .expect("Error getting the parent")
                        .to_owned()
                        .join(fs_config.error_file_name.to_owned()),
                    format!(
                        "Query file with inode {inode} not found",
                        inode = blutils_config.query_file_id
                    )
                    .as_str(),
                ) {
                    panic!("Failed to write the error file: {err}");
                };

                continue;
            }
        };

        let query_file =
            FileOrStdin::from_file(&query_file_path.to_str().unwrap());

        // ? -------------------------------------------------------------------
        // ? Build the output file path
        // ? -------------------------------------------------------------------

        let output_file = parent
            .to_owned()
            .join(fs_config.output_directory.to_owned().as_str())
            .join(fs_config.results_file_name.to_owned().as_str());

        if let Err(err) = ExecutionMsg::write_file(
            &parent
                .to_owned()
                .join(fs_config.running_file_name.to_owned()),
            format!(
                "Processing the query file {query_file:?} with model {model_id:?}",
                query_file = query_file_path.file_name().to_owned(),
                model_id = analysis_model.id
            )
            .as_str(),
        ) {
            panic!("Failed to write the error file: {err}");
        };

        // ? -------------------------------------------------------------------
        // ? Place sequences
        // ? -------------------------------------------------------------------

        if let Err(err) = place_sequences(
            query_file,
            &tree_model,
            &output_file,
            &None,
            &None,
            &true,
            &blutils_config.output_format,
            &blutils_config.remove_intersection,
        ) {
            if let Err(err) = ExecutionMsg::write_file(
                &parent
                    .to_owned()
                    .join(fs_config.error_file_name.to_owned()),
                format!(
                    "Failed to process the query file {query_file:?} with model {model_id:?}: {err}",
                    query_file = query_file_path.file_name().to_owned(),
                    model_id = analysis_model.id
                )
                .as_str(),
            ) {
                panic!("Failed to write the error file: {err}");
            };

            continue;
        }

        // ? -------------------------------------------------------------------
        // ? Write response
        // ? -------------------------------------------------------------------

        if let Err(err) = ExecutionMsg::write_file(
            &parent.join(fs_config.success_file_name.to_owned()),
            format!(
                "Query file {query_file:?} processed successfully",
                query_file = query_file_path.file_name()
            )
            .as_str(),
        ) {
            panic!("Failed to write the error file: {err}");
        };
    }
}

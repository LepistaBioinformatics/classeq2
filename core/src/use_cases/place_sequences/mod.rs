mod place_sequence;
use place_sequence::*;

use super::shared::write_or_append_to_file::write_or_append_to_file;
use crate::domain::dtos::placement_response::PlacementStatus;
use crate::domain::dtos::{
    file_or_stdin::FileOrStdin, output_format::OutputFormat,
    placement_response::PlacementResponse, tree::Tree,
};

use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::channel;
use std::{
    fs::{create_dir, remove_file},
    path::PathBuf,
    time::Duration,
};
use tracing::{debug, warn};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlacementTime {
    pub sequence: String,
    pub milliseconds_time: Duration,
}

#[tracing::instrument(
    name = "Placing multiple sequences",
    skip(query_sequence, tree)
)]
pub fn place_sequences(
    query_sequence: FileOrStdin,
    tree: &Tree,
    out_file: &PathBuf,
    max_iterations: &Option<i32>,
    overwrite: &bool,
    output_format: &OutputFormat,
    threads: &usize,
) -> Vec<PlacementTime> {
    // ? -----------------------------------------------------------------------
    // ? Create a thread pool configured globally
    // ? -----------------------------------------------------------------------

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads.to_owned())
        .build_global()
        .expect("Error creating thread pool");

    // ? -----------------------------------------------------------------------
    // ? Build the output paths
    // ? -----------------------------------------------------------------------

    let mut out_dir_path = PathBuf::from(out_file);
    out_dir_path.set_extension(match output_format {
        OutputFormat::Yaml => "yaml",
        OutputFormat::Jsonl => "jsonl",
    });

    let out_dir = out_dir_path.parent().unwrap();

    if !out_dir.exists() {
        let _ = create_dir(out_dir);
    }

    if out_dir_path.exists() {
        if !overwrite {
            panic!(
                "Could not overwrite existing file {:?} when overwrite option is `false`.", 
                out_dir_path
            );
        }

        match remove_file(out_dir_path.clone()) {
            Err(err) => panic!("Could not remove file given {}", err),
            Ok(_) => warn!("Output file overwritten!"),
        };
    };

    // ? -----------------------------------------------------------------------
    // ? Run the placement
    // ? -----------------------------------------------------------------------

    let (writer, file) = write_or_append_to_file(out_dir_path.as_path());
    let (sender, receiver) = channel();
    let _ = query_sequence.sequence_content_by_channel(sender);

    receiver
        .into_iter()
        .par_bridge()
        .map(|sequence| {
            debug!("Processing {:?}", sequence.header_content());

            let time = std::time::Instant::now();

            match place_sequence(
                &sequence.header().to_owned(),
                &sequence.sequence().to_owned(),
                &tree,
                &max_iterations,
                None,
            ) {
                Err(err) => panic!("Error placing sequence: {err}"),
                Ok(placement) => {
                    debug!("Placed sequence: {:?}", placement);

                    let output = PlacementResponse::new(
                        sequence.header_content().to_string(),
                        placement.to_string(),
                        match placement {
                            PlacementStatus::Unclassifiable => None,
                            other => Some(other),
                        },
                    );

                    let output_content = match output_format {
                        OutputFormat::Yaml => {
                            let content = serde_yaml::to_string(&output)
                                .expect("Error serializing YAML response");

                            format!("---\n{content}")
                        }
                        OutputFormat::Jsonl => {
                            let content = serde_json::to_string(&output)
                                .expect("Error serializing JSON response");

                            format!("{content}\n")
                        }
                    };

                    if let Err(err) = writer(
                        output_content,
                        file.try_clone().expect(
                            "Unexpected error detected on write blast result",
                        ),
                    ) {
                        panic!("Error writing to file: {err}")
                    }

                    PlacementTime {
                        sequence: sequence.header_content().to_string(),
                        milliseconds_time: time.elapsed(),
                    }
                }
            }
        })
        .collect()

    /* match query_sequence.sequence_content_by_channel(sender) {
        Err(err) => panic!("Error reading sequence content: {err}"),
        Ok(source_sequences) => source_sequences
            .par_iter()
            .map(|sequence| {
                debug!("Processing {:?}", sequence.header_content());

                let time = std::time::Instant::now();

                match place_sequence(
                    &sequence.header().to_owned(),
                    &sequence.sequence().to_owned(),
                    &tree,
                    &max_iterations,
                    None,
                ) {
                    Err(err) => {
                        panic!("Error placing sequence: {err}");
                    }
                    Ok(placement) => {
                        debug!("Placed sequence: {:?}", placement);

                        let output = PlacementResponse::new(
                            sequence.header_content().to_string(),
                            placement.to_string(),
                            placement,
                        );

                        let output_content = match output_format {
                            OutputFormat::Yaml => {
                                let content = serde_yaml::to_string(&output)
                                    .expect("Error serializing YAML response");

                                format!("---\n{content}")
                            }
                            OutputFormat::Jsonl => {
                                let content = serde_json::to_string(&output)
                                    .expect("Error serializing JSON response");

                                format!("{content}\n")
                            }
                        };

                        if let Err(err) = writer(output_content, file.try_clone().expect(
                            "Unexpected error detected on write blast result",
                        )) {
                             panic!("Error writing to file: {err}")
                        }

                        PlacementTime {
                            sequence: sequence.header_content().to_string(),
                            milliseconds_time: time.elapsed(),
                        }
                    }
                }
            }).collect(),
    } */
}

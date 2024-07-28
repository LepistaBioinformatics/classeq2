mod dtos;
mod place_sequence;
mod update_introspection_node;

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use place_sequence::*;

use super::shared::write_or_append_to_file::write_or_append_to_file;
use crate::domain::dtos::placement_response::PlacementStatus;
use crate::domain::dtos::{
    file_or_stdin::FileOrStdin, output_format::OutputFormat,
    placement_response::PlacementResponse, telemetry_code::TelemetryCode,
    tree::Tree,
};

use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::channel;
use std::{
    fs::{create_dir, remove_file},
    path::PathBuf,
    time::Duration,
};
use tracing::{debug, debug_span, warn};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlacementTime {
    pub sequence: String,
    pub milliseconds_time: Duration,
}

#[tracing::instrument(
    name = "PlacingMultipleSequences",
    skip(query_sequence, tree),
    fields(
        run_id = Uuid::new_v4().to_string().replace("-", "")
    )
)]
pub fn place_sequences(
    query_sequence: FileOrStdin,
    tree: &Tree,
    out_file: &PathBuf,
    max_iterations: &Option<i32>,
    min_match_coverage: &Option<f64>,
    overwrite: &bool,
    output_format: &OutputFormat,
    remove_intersection: &Option<bool>,
) -> Result<Vec<PlacementTime>, MappedErrors> {
    // ? -----------------------------------------------------------------------
    // ? Configure the logging span
    // ? -----------------------------------------------------------------------

    debug!(
        code = TelemetryCode::UCPLACE0001.to_string(),
        "Start multiple sequences placement"
    );

    // ? -----------------------------------------------------------------------
    // ? Build the output paths
    // ? -----------------------------------------------------------------------

    let mut out_file_path = out_file.to_owned();
    let mut err_file_path = out_file.to_owned();

    out_file_path.set_extension(match output_format {
        OutputFormat::Yaml => "yaml",
        OutputFormat::Jsonl => "jsonl",
    });

    err_file_path.set_extension("error");

    let out_dir = out_file_path.parent().unwrap();

    if !out_dir.exists() {
        let _ = create_dir(out_dir);
    }

    if out_file_path.exists() {
        if !overwrite {
            return use_case_err(format!(
                "Could not overwrite existing file {:?} when overwrite option is `false`.", 
                out_file_path
            )).as_error();
        }

        match remove_file(out_file_path.clone()) {
            Err(err) => {
                return use_case_err(format!(
                    "Could not remove file given {err}"
                ))
                .as_error()
            }
            Ok(_) => warn!("Output file overwritten!"),
        };
    };

    // ? -----------------------------------------------------------------------
    // ? Run the placement
    // ? -----------------------------------------------------------------------

    let (result_writer, result_file) =
        write_or_append_to_file(out_file_path.as_path());

    let (error_writer, error_file) =
        write_or_append_to_file(err_file_path.as_path());

    let (sender, receiver) = channel();
    let _ = query_sequence.sequence_content_by_channel(sender);

    let responses = receiver
        .into_iter()
        .par_bridge()
        .map(|sequence| {
            let header = sequence.header_content();

            let span = debug_span!(
                "PlacingSequence",
                tree_id = tree.id.to_string().replace("-", ""),
                header = header.to_string(),
            );

            let _span_guard = span.enter();

            debug!(
                code = TelemetryCode::UCPLACE0003.to_string(),
                query = header,
                query_id =
                    Uuid::new_v3(&Uuid::NAMESPACE_DNS, header.as_bytes())
                        .to_string()
                        .replace("-", ""),
                "Start placing sequence: {header}",
                header = header
            );

            let time = std::time::Instant::now();

            match place_sequence(
                &sequence.sequence().to_owned(),
                &tree,
                &max_iterations,
                &min_match_coverage,
                &remove_intersection,
            ) {
                Err(err) => {
                    if let Err(err) = error_writer(
                        err.to_string(),
                        error_file.try_clone().expect(
                            "Unexpected error detected on write blast result",
                        ),
                    ) {
                        panic!("Error writing to file: {err}")
                    };
                }
                Ok(placement) => {
                    let output = PlacementResponse::new(
                        sequence.header_content().to_string(),
                        placement.to_string(),
                        match placement {
                            PlacementStatus::Unclassifiable(_) => None,
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

                    if let Err(err) = result_writer(
                        output_content,
                        result_file.try_clone().expect(
                            "Unexpected error detected on write blast result",
                        ),
                    ) {
                        panic!("Error writing to file: {err}")
                    };
                }
            }

            debug!(
                code = TelemetryCode::UCPLACE0004.to_string(),
                "Sequence placed"
            );

            PlacementTime {
                sequence: sequence.header_content().to_string(),
                milliseconds_time: time.elapsed(),
            }
        })
        .collect();

    debug!(
        code = TelemetryCode::UCPLACE0002.to_string(),
        "End multiple sequences placement"
    );

    Ok(responses)
}

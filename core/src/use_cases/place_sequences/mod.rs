mod _dtos;
mod clade_from_placement_status;
mod place_sequence;
mod update_introspection_node;

use clade_from_placement_status::*;
use place_sequence::*;

use super::shared::write_or_append_to_file::write_or_append_to_file;
use crate::domain::dtos::{
    file_or_stdin::FileOrStdin,
    output_format::OutputFormat,
    placement_response::{PlacementResponse, PlacementStatus},
    telemetry_code::TelemetryCode,
    tree::Tree,
};

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    fs::{create_dir, remove_file},
    path::PathBuf,
    sync::mpsc::channel,
    time::Duration,
};
use tracing::{debug, trace_span, warn};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlacementTime {
    pub sequence: String,
    pub milliseconds_time: Duration,
}

#[tracing::instrument(
    name = "PlacingMultipleSequences",
    skip(query_sequence, tree, parent_span),
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
    parent_span: &Option<&tracing::Span>,
) -> Result<Vec<PlacementTime>, MappedErrors> {
    // ? -----------------------------------------------------------------------
    // ? Configure the logging span
    // ? -----------------------------------------------------------------------

    if let Some(span) = parent_span {
        let span =
            trace_span!(parent: span.to_owned(), "PlaceMultipleSequences");
        let _span_guard = span.enter();
    }

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

    let annotations = tree.annotations.to_owned();

    let responses = receiver
        .into_iter()
        .par_bridge()
        .map(|sequence| {
            let header = sequence.header_content();

            let span = trace_span!(
                parent: parent_span.unwrap_or(&tracing::Span::current()),
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
                &sequence.header().to_owned(),
                &sequence.sequence().to_owned(),
                &tree,
                &max_iterations,
                &min_match_coverage,
                &remove_intersection,
                parent_span,
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
                    let mut output = PlacementResponse::new(
                        sequence.header_content().to_string(),
                        placement.to_string(),
                        match placement {
                            PlacementStatus::Unclassifiable(_) => None,
                            other => Some(other),
                        },
                    );

                    if let Some(annotations) = annotations.to_owned() {
                        let optional_clade =
                            clade_from_placement_status(output.placement());

                        let node_annotations =
                            if let Some(clade) = optional_clade {
                                let tree_node = if let Some(node) =
                                    tree.root.get_node_by_id(clade)
                                {
                                    node.get_path_to_root(&tree.root)
                                        .into_iter()
                                        .collect::<Vec<_>>()
                                } else {
                                    vec![]
                                };

                                let mut records = annotations
                                    .to_owned()
                                    .iter()
                                    .filter(|item| {
                                        tree_node.contains(&(item.clade as u64))
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>();

                                if records.len() > 0 {
                                    records.sort_by(|a, b| {
                                        a.clade.partial_cmp(&b.clade).unwrap()
                                    });
                                    Some(records)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                        output = output.with_annotation(node_annotations);
                    }

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

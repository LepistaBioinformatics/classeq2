mod _dtos;
mod clade_from_placement_status;
mod place_sequence;
mod update_introspection_node;

use clade_from_placement_status::*;
use place_sequence::*;

use crate::domain::dtos::{
    annotation::Annotation,
    clade::Clade,
    file_or_stdin::FileOrStdin,
    kmer_index::KmerIndex,
    output_format::OutputFormat,
    placement_response::{PlacementResponse, PlacementStatus},
    telemetry_code::TelemetryCode,
    tree::Tree,
};

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, remove_file, rename, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{mpsc::sync_channel, Mutex},
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

    if let Err(err) = create_dir_all(out_dir) {
        return use_case_err(format!(
            "Could not create output directory {out_dir:?}: {err}"
        ))
        .as_error();
    }

    let reader = match query_sequence.open_reader() {
        Ok(reader) => reader,
        Err(err) => {
            return use_case_err(format!(
                "Could not read query sequences: {err}"
            ))
            .as_error()
        }
    };

    let overwritten = out_file_path.exists();

    if overwritten && !overwrite {
        return use_case_err(format!(
            "Could not overwrite existing file {:?} when overwrite option is `false`.", 
            out_file_path
        )).as_error();
    }

    // ? -----------------------------------------------------------------------
    // ? Run the placement
    //
    // The queries are read while they are placed, so the records go to
    // `.partial` files first. They replace the outputs only when the whole
    // input was read: a malformed input leaves no output and keeps the
    // previous one.
    //
    // ? -----------------------------------------------------------------------

    let partial_out_path = partial_path(&out_file_path);
    let partial_err_path = partial_path(&err_file_path);
    let result_file = create_partial(&partial_out_path)?;
    let error_file = create_partial(&partial_err_path)?;

    let frozen = frozen_index(tree);
    let index = tree.index.as_deref().or(frozen.as_ref()).unwrap();
    let annotations = tree.annotations.as_deref();
    let parents = annotations.map(|_| parent_ids(&tree.root));

    let (sender, receiver) =
        sync_channel(4 * rayon::current_num_threads().max(1));

    let (responses, read_result) = std::thread::scope(|scope| {
        let producer =
            scope.spawn(move || FileOrStdin::send_sequences(reader, sender));

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
                    sequence.header(),
                    sequence.sequence(),
                    tree,
                    index,
                    max_iterations,
                    min_match_coverage,
                    remove_intersection,
                    parent_span,
                ) {
                    Err(err) => {
                        write_record(&error_file, format!("{header}\t{err}\n"));
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

                        if let (Some(annotations), Some(parents)) =
                            (annotations, &parents)
                        {
                            debug!(
                                code = TelemetryCode::UCPLACE00020.to_string(),
                                "Annotating sequence: {header}",
                                header = header
                            );

                            let node_annotations =
                                clade_from_placement_status(output.placement())
                                    .and_then(|clade| {
                                        annotate(annotations, parents, clade)
                                    });

                            output = output.with_annotation(node_annotations);
                        }

                        let output_content = match output_format {
                            OutputFormat::Yaml => {
                                let content = serde_saphyr::to_string(&output)
                                    .expect("Error serializing YAML response");

                                format!("---\n{content}")
                            }
                            OutputFormat::Jsonl => {
                                let content = serde_json::to_string(&output)
                                    .expect("Error serializing JSON response");

                                format!("{content}\n")
                            }
                        };

                        write_record(&result_file, output_content);
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
            .collect::<Vec<PlacementTime>>();

        (
            responses,
            producer.join().expect("The query reader panicked"),
        )
    });

    for file in [result_file, error_file] {
        if let Err(err) = file.into_inner().unwrap().flush() {
            panic!("Error writing to file: {err}")
        }
    }

    if let Err(err) = read_result {
        let _ = remove_file(&partial_out_path);
        let _ = remove_file(&partial_err_path);

        return use_case_err(format!("Could not read query sequences: {err}"))
            .as_error();
    }

    for (from, to) in [
        (&partial_out_path, &out_file_path),
        (&partial_err_path, &err_file_path),
    ] {
        if let Err(err) = rename(from, to) {
            return use_case_err(format!(
                "Could not write output file {to:?}: {err}"
            ))
            .as_error();
        }
    }

    if overwritten {
        warn!("Output file overwritten!");
    }

    debug!(
        code = TelemetryCode::UCPLACE0002.to_string(),
        "End multiple sequences placement"
    );

    Ok(responses)
}

/// The index to place against: the tree's own, or one frozen from its
/// `kmers_map` for trees built in memory.
fn frozen_index(tree: &Tree) -> Option<KmerIndex> {
    match tree.index {
        Some(_) => None,
        None => Some(KmerIndex::freeze(
            tree.kmers_map
                .as_ref()
                .expect("The tree does not have a kmers map."),
        )),
    }
}

fn partial_path(path: &Path) -> PathBuf {
    let mut partial = path.as_os_str().to_owned();
    partial.push(".partial");
    PathBuf::from(partial)
}

fn create_partial(path: &Path) -> Result<Mutex<BufWriter<File>>, MappedErrors> {
    match File::create(path) {
        Ok(file) => Ok(Mutex::new(BufWriter::new(file))),
        Err(err) => use_case_err(format!(
            "Could not create output file {path:?}: {err}"
        ))
        .as_error(),
    }
}

fn write_record(file: &Mutex<BufWriter<File>>, content: String) {
    if let Err(err) = file.lock().unwrap().write_all(content.as_bytes()) {
        panic!("Error writing to file: {err}")
    }
}

/// The `parent` field of every clade, by id, in the order
/// `Clade::get_node_by_id` finds them.
fn parent_ids(root: &Clade) -> HashMap<u64, Option<u64>> {
    let mut parents = HashMap::new();
    let mut stack = vec![root];

    while let Some(clade) = stack.pop() {
        parents.entry(clade.id).or_insert(clade.parent);

        if let Some(children) = &clade.children {
            stack.extend(children.iter().rev());
        }
    }

    parents
}

/// The annotations of the clades on the path from `clade` to the root, as
/// `Clade::get_path_to_root` gives it, sorted by clade.
fn annotate(
    annotations: &[Annotation],
    parents: &HashMap<u64, Option<u64>>,
    clade: u64,
) -> Option<Vec<Annotation>> {
    let mut path = HashSet::new();

    if parents.contains_key(&clade) {
        let mut current = Some(clade);

        while let Some(id) = current {
            if !path.insert(id) {
                break;
            }

            current = parents.get(&id).copied().flatten();

            if let Some(parent) = current {
                if !parents.contains_key(&parent) {
                    path.insert(parent);
                    break;
                }
            }
        }
    }

    let mut records = annotations
        .iter()
        .filter(|item| path.contains(&(item.clade as u64)))
        .cloned()
        .collect::<Vec<_>>();

    if records.is_empty() {
        return None;
    }

    records.sort_by(|a, b| a.clade.partial_cmp(&b.clade).unwrap());
    Some(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dtos::clade::NodeType;

    fn clade(id: u64, parent: Option<u64>, children: Vec<Clade>) -> Clade {
        Clade {
            id,
            parent,
            kind: if parent.is_none() {
                NodeType::Root
            } else if children.is_empty() {
                NodeType::Leaf
            } else {
                NodeType::Node
            },
            name: None,
            support: None,
            length: None,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    #[test]
    #[should_panic(expected = "The tree does not have a kmers map.")]
    fn place_without_kmers_map_panics() {
        let tree = Tree::new(
            Uuid::nil(),
            "empty".to_string(),
            0.0,
            clade(0, None, vec![]),
        );

        let _ = frozen_index(&tree);
    }

    #[test]
    fn parent_ids_give_the_same_path_as_get_path_to_root() {
        // 0 -> {1 -> {2, 3 -> {4, 5}}, 6}; node 7 claims a missing parent.
        let root = clade(
            0,
            None,
            vec![
                clade(
                    1,
                    Some(0),
                    vec![
                        clade(2, Some(1), vec![]),
                        clade(
                            3,
                            Some(1),
                            vec![
                                clade(4, Some(3), vec![]),
                                clade(5, Some(3), vec![]),
                            ],
                        ),
                    ],
                ),
                clade(6, Some(0), vec![clade(7, Some(99), vec![])]),
            ],
        );

        let parents = parent_ids(&root);
        let annotations = (0..8)
            .chain([99])
            .map(|id| Annotation {
                clade: id,
                meta: None,
            })
            .collect::<Vec<_>>();

        for id in 0..9 {
            let expected = root
                .get_node_by_id(id)
                .map(|node| node.get_path_to_root(&root))
                .unwrap_or_default();

            let got = annotate(&annotations, &parents, id)
                .unwrap_or_default()
                .into_iter()
                .map(|item| item.clade as u64)
                .collect::<HashSet<u64>>();

            assert_eq!(got, expected, "clade {id}");
        }
    }
}

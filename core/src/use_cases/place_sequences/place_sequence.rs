use super::{
    _dtos::IntrospectionUpdateResponse::*,
    update_introspection_node::update_introspection_node,
};
use crate::domain::dtos::{
    adherence_test::AdherenceTest,
    clade::Clade,
    placement_response::PlacementStatus::{self, *},
    sequence::{SequenceBody, SequenceHeader},
    telemetry_code::TelemetryCode,
    tree::Tree,
};

use mycelium_base::{
    dtos::UntaggedParent,
    utils::errors::{use_case_err, MappedErrors},
};
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, ParallelBridge,
    ParallelIterator,
};
use std::collections::{HashMap, HashSet};
use tracing::{debug_span, info, trace, trace_span, Span};

/// Place a sequence in the tree.
///
/// This function tries to place a sequence in the tree using the overlapping
/// kmers. The function uses a recursive strategy to traverse the tree and
/// evaluate the adherence of the query sequence to the clades.
#[tracing::instrument(
    name = "PlaceSingleSequence",
    skip_all,
    fields(
        query.kmers.count = tracing::field::Empty,
        query.kmers.treeMatches = tracing::field::Empty,
        query.kmers.buildTime = tracing::field::Empty,
        subject.kmers.queryMatches = tracing::field::Empty,
        subject.kmers.buildTime = tracing::field::Empty,
        subject.kmers.children = tracing::field::Empty,
    )
)]
pub(super) fn place_sequence(
    header: &SequenceHeader,
    sequence: &SequenceBody,
    tree: &Tree,
    max_iterations: &Option<i32>,
    min_match_coverage: &Option<f64>,
    remove_intersection: &Option<bool>,
    parent_span: &Option<&tracing::Span>,
) -> Result<PlacementStatus, MappedErrors> {
    // ? -----------------------------------------------------------------------
    // ? Configure the logging span
    // ? -----------------------------------------------------------------------

    if let Some(span) = parent_span {
        let span = trace_span!(parent: span.to_owned(), "PlaceSingleSequences");
        let _span_guard = span.enter();
    }

    // ? -----------------------------------------------------------------------
    // ? Start placement
    // ? -----------------------------------------------------------------------

    let remove_intersection = remove_intersection.unwrap_or(false);
    let max_iterations = max_iterations.unwrap_or(1000);

    let min_match_coverage = if let Some(value) = min_match_coverage {
        match value.to_owned() {
            value if value > 1.0 => 1.0,
            value if value < 0.0 => 0.0,
            value => value,
        }
    } else {
        0.7
    };

    let kmers_map = tree
        .kmers_map
        .as_ref()
        .expect("The tree does not have a kmers map.");

    // ? -----------------------------------------------------------------------
    // ? Build and validate query kmers
    // ? -----------------------------------------------------------------------

    let time = std::time::Instant::now();
    let query_kmers =
        kmers_map.build_kmer_from_string(sequence.seq().to_string(), None);

    Span::current()
        .record("query.kmers.count", &Some(query_kmers.len() as i32));

    Span::current().record(
        "query.kmers.buildTime",
        &Some(format!("{:?}", time.elapsed())),
    );

    if query_kmers.len() < 2 {
        return use_case_err("The sequence does not contain enough kmers.")
            .with_code(TelemetryCode::UCPLACE0005.to_string().as_str())
            .as_error();
    }

    trace!(
        code = TelemetryCode::UCPLACE0005.to_string(),
        "Query kmers built successfully"
    );

    // ? -----------------------------------------------------------------------
    // ? Sub-sampling kmers_map from the query_kmers
    //
    // Here the kmers_map is sub-sampled to only contain the kmers present in
    // the query sequence. This is done to reduce the number of kmers to be
    // processed.
    //
    // ? -----------------------------------------------------------------------

    let query_distinct_kmers = query_kmers
        .iter()
        .map(|(_, hash)| *hash)
        .collect::<HashSet<u64>>()
        .len();

    let query_kmers_map = kmers_map.get_overlapping_hashed_kmers(query_kmers);

    let query_kmers_len = query_kmers_map
        .get_map()
        .values()
        .par_bridge()
        .map(|i| i.0.len())
        .sum::<usize>();

    Span::current()
        .record("query.kmers.treeMatches", &Some(query_kmers_len as i32));

    if query_kmers_len == 0 {
        let msg = format!(
            "Query sequence {query:?} may not be related to the phylogeny",
            query = header
        );

        info!(code = TelemetryCode::UCPLACE0006.to_string(), msg);

        return Ok(Unclassifiable(msg));
    }

    trace!(
        code = TelemetryCode::UCPLACE0006.to_string(),
        "Query kmers map built successfully"
    );

    // ? -----------------------------------------------------------------------
    // ? Build the root kmers map
    //
    // The root kmers map is built using the kmers from the root node. This map
    // is used to evaluate the adherence of the query sequence to the root node.
    //
    // ? -----------------------------------------------------------------------

    let time = std::time::Instant::now();

    let introspection_kmers = match query_kmers_map
        .get_minimized_hashes_with_node(tree.root.id)
    {
        None => {
            let msg = "Query sequence has no overlapping kmers with the \
                reference tree";
            trace!(code = TelemetryCode::UCPLACE0007.to_string(), msg);
            return Ok(Unclassifiable(msg.to_string()));
        }
        Some(kmers) => query_kmers_map.get_overlapping_minimized_hashes(kmers),
    };

    Span::current().record(
        "subject.kmers.queryMatches",
        &Some(
            introspection_kmers
                .get_map()
                .into_iter()
                .map(|(_, v)| v.0.len() as i32)
                .sum::<i32>(),
        ),
    );

    Span::current().record(
        "subject.kmers.buildTime",
        &Some(format!("{:?}", time.elapsed())),
    );

    trace!(
        code = TelemetryCode::UCPLACE0007.to_string(),
        "Root kmers map built successfully"
    );

    // ? -----------------------------------------------------------------------
    // ? Start the children clades with the root
    //
    // Symbol: 🌿
    //
    // This object should be updated during the search process. The symbol 🌿
    // indicate wether this object is updated.
    //
    // ? -----------------------------------------------------------------------

    let mut children = if let Some(children) = tree.root.to_owned().children {
        children
    } else {
        return use_case_err(
            "The root node does not have children. This is unexpected.",
        )
        .as_error();
    };

    let mut iteration = 0;

    // ? -----------------------------------------------------------------------
    // ? Set the initial parent
    //
    // Symbol: 🍁
    //
    // The clade object is used to store the current clade as a parent being
    // evaluated. The symbol 🍁 indicate wether this object is updated.
    //
    // ? -----------------------------------------------------------------------

    let mut parent = tree.root.to_owned();

    Span::current()
        .record("subject.kmers.children", &Some(children.len() as i32));

    //
    // This rule is used to determine if the child node has enough kmers
    // to be considered for the adherence test. If the child node does
    // not have enough kmers, the search process should perform a young
    // return with `Unclassifiable` status.
    //
    let expected_min_clade_coverage =
        (query_distinct_kmers as f64 * min_match_coverage).round();

    trace!(
        code = TelemetryCode::UCPLACE0008.to_string(),
        "Expected min clade coverage (base {base}): {expected}",
        base = min_match_coverage,
        expected = expected_min_clade_coverage
    );

    let introspection_coverage = introspection_kmers
        .get_map()
        .into_par_iter()
        .map(|i| i.1 .0.len())
        .sum::<usize>();

    if introspection_coverage < expected_min_clade_coverage as usize {
        let msg =
            format!("Insufficient kmers coverage: {introspection_coverage}");

        trace!(code = TelemetryCode::UCPLACE0008.to_string(), msg);

        return Ok(Unclassifiable(msg));
    }

    // ? -----------------------------------------------------------------------
    // ? Fire the search loop
    //
    // Each iteration of the loop is a new introspection level. During the
    // search loop the algorithm try to place the sequence into a one or more
    // clades. Case the search loop reach the maximum number of iterations, the
    // search is considered maxed out and this function should return a
    // `use_case_err`.
    //
    // SYMBOLS:
    //  - 🍁: clade update
    //  - 🌿: children update
    //  - 🟢: introspect to the next tree level
    //  - 🔴: young return
    //  - ✅: conclusive identity found
    //
    // ? -----------------------------------------------------------------------

    trace!(
        code = TelemetryCode::UCPLACE0009.to_string(),
        "Starting tree introspection"
    );

    loop {
        iteration += 1;

        let iteration_span = debug_span!(
            "Introspection",
            code = TelemetryCode::UCPLACE0010.to_string(),
            level = iteration,
            clade_id = parent.id
        );

        let _iteration_span_guard = iteration_span.enter();

        // ? -------------------------------------------------------------------
        // ? Start the placement process
        // ? -------------------------------------------------------------------

        if iteration > max_iterations {
            return use_case_err(
                "The maximum number of iterations has been reached.",
            )
            .with_code(TelemetryCode::UCPLACE0010.to_string().as_str())
            .as_error();
        }

        // ? -------------------------------------------------------------------
        // ? PHASE 1: Generate clade proposals for the current level
        //
        // Clade proposals include all possible clades that a given query
        // sequence can be placed. The clade proposals are generated by
        // comparing the query sequence to the sibling clades.
        //
        // ? -------------------------------------------------------------------
        let clade_proposals = {
            let children_lengths_time = std::time::Instant::now();

            //
            // Aggregate children kmer lengths. This action is necessary to
            // determine the adherence of the query sequence to the sibling
            // clades.
            //
            let mut children_kmers = children
                .par_iter()
                .filter_map(|record| {
                    if record.is_leaf() {
                        return None;
                    }

                    match introspection_kmers
                        .get_hashed_kmers_with_node(record.id)
                    {
                        None => None,
                        Some(kmers) => Some((kmers, record)),
                    }
                })
                .collect::<Vec<(HashSet<u64>, &Clade)>>();

            children_kmers.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

            trace!(
                code = TelemetryCode::UCPLACE0012.to_string(),
                "Level clades (runtime {time}): {lengths}",
                time = format!("{:?}", children_lengths_time.elapsed()),
                lengths = children_kmers
                    .to_owned()
                    .iter()
                    .map(|(kmers, clade)| {
                        format!("{} ({})", clade.id, kmers.len())
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            );

            let clade_proposals_time = std::time::Instant::now();

            let clade_proposals = children_kmers
                .to_owned()
                .into_iter()
                .par_bridge()
                .filter_map(|(kmers, clade)| {
                    let rest: Vec<_> = children_kmers
                        .par_iter()
                        .filter_map(|(rest_kmers, nested_clade)| {
                            if nested_clade.id == clade.id {
                                return None;
                            }

                            Some(rest_kmers.to_owned())
                        })
                        .collect();

                    if rest.is_empty() {
                        return Some(AdherenceTest {
                            clade: UntaggedParent::Record(clade.to_owned()),
                            one: kmers.len() as i32,
                            rest: 0,
                        });
                    }

                    let rest_len = rest
                        .iter()
                        .map(|i| i.to_owned())
                        .flatten()
                        .collect::<HashSet<u64>>();

                    let (one_kmers, rest_kmers) = match remove_intersection {
                        true => (
                            kmers
                                .difference(&rest_len)
                                .map(|i| *i)
                                .collect::<HashSet<_>>(),
                            rest_len
                                .difference(&kmers)
                                .map(|i| *i)
                                .collect::<HashSet<_>>(),
                        ),
                        false => (kmers.to_owned(), rest_len.to_owned()),
                    };

                    trace!(
                        code = TelemetryCode::UCPLACE0013.to_string(),
                        "Clade {id}: one {one_kmers} vs rest {rest_kmers}",
                        id = clade.id,
                        one_kmers = one_kmers.len(),
                        rest_kmers = rest_kmers.len(),
                    );

                    Some(AdherenceTest {
                        clade: UntaggedParent::Record(clade.to_owned()),
                        one: one_kmers.len() as i32,
                        rest: rest_kmers.len() as i32,
                    })
                })
                .filter_map(|adherence| {
                    if adherence.one > adherence.rest as i32 {
                        Some(adherence.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<AdherenceTest>>();

            trace!(
                code = TelemetryCode::UCPLACE0014.to_string(),
                "Available proposals (runtime {time}): {proposals}",
                time = format!("{:?}", clade_proposals_time.elapsed()),
                proposals = clade_proposals.len()
            );

            clade_proposals
        };

        // ? -------------------------------------------------------------------
        // ? PHASE 2: Evaluate proposals
        //
        // The algorithm evaluates the proposals and decide the next step.
        //
        // ? -------------------------------------------------------------------
        {
            // ? ---------------------------------------------------------------
            // ? Case 1: No proposals
            // ? Events: 🔴
            //
            // If none of the proposed clades have a higher adherence than the
            // sibling clades, the search are considered maxed out, than, the
            // query sequence is placed at the current clade.
            //
            // ? ---------------------------------------------------------------
            if clade_proposals.is_empty() {
                if iteration == 1 {
                    let msg = "Tree introspection not possible. Query \
                        sequence has no overlapping kmers with the reference \
                        tree";
                    trace!(code = TelemetryCode::UCPLACE0011.to_string(), msg);
                    return Ok(Unclassifiable(msg.to_string()));
                }

                trace!(
                    code = TelemetryCode::UCPLACE0015.to_string(),
                    "No proposals found. Max resolution reached at clade {clade_id}",
                    clade_id = parent.id
                );

                return Ok(MaxResolutionReached(
                    parent.id,
                    "LCA Accepted".to_string(),
                ));
            }

            // ? ---------------------------------------------------------------
            // ? Case 2: One clade proposal
            // ? Events: 🟢 or ✅
            //
            // If only one clade has a higher adherence than the sibling clades,
            // the query sequence is placed at the current clade.
            //
            // ? ---------------------------------------------------------------
            if clade_proposals.len() == 1 {
                let adherence: AdherenceTest = match clade_proposals.first() {
                    Some(adherence) => adherence.to_owned(),
                    None => {
                        return use_case_err(
                            "The filtered proposals list is empty. This is \
                            unexpected.",
                        )
                        .as_error();
                    }
                };

                //
                // 🍁 1st clade update
                // 🌿 1st children update
                //
                (parent, children) =
                    match update_introspection_node(&adherence)? {
                        //
                        // ✅ Return a status if the search loop is finished with
                        // a conclusive identity.
                        //
                        Return(status) => {
                            return Ok(status);
                        }
                        //
                        // 🟢 Continue the search loop with the new parent and
                        // children
                        //
                        Continue(a, b) => (a, b),
                    };

                continue;
            }

            // ? ---------------------------------------------------------------
            // ? Case 3: No proposals
            // ? Events: ✅ or 🟢
            //
            // If more than one clade has a higher adherence than the sibling
            // clades, the search is considered inconclusive. The query sequence
            // is placed at the current clade.
            //
            // ? ---------------------------------------------------------------
            if clade_proposals.len() > 1 {
                trace!(
                    code = TelemetryCode::UCPLACE0018.to_string(),
                    "Multiple proposals found. Clade {parent} selected",
                    parent = parent.id
                );

                let fold_proposals = clade_proposals.iter().fold(
                    HashMap::<i32, Vec<AdherenceTest>>::new(),
                    |mut acc, a| {
                        acc.entry(a.one - a.rest)
                            .or_insert(vec![])
                            .push(a.to_owned());

                        acc
                    },
                );

                let max_diff_key = fold_proposals.keys().max().unwrap();
                let max_diff_value = fold_proposals.get(max_diff_key).unwrap();

                if max_diff_value.len() == 1 {
                    let adherence: &AdherenceTest = match max_diff_value.first()
                    {
                        Some(adherence) => adherence,
                        None => {
                            return use_case_err(
                                "The filtered proposals list is empty. This is \
                                unexpected."
                            ).as_error();
                        }
                    };

                    //
                    // 🍁 2nd clade update
                    // 🌿 2nd children update
                    //
                    (parent, children) =
                        match update_introspection_node(&adherence)? {
                            //
                            // ✅ Return a status if the search loop is finished
                            // with a conclusive identity.
                            //
                            Return(status) => {
                                return Ok(status);
                            }
                            //
                            // 🟢 Continue the search loop with the new parent
                            // and children
                            //
                            Continue(a, b) => (a, b),
                        };

                    continue;
                }

                //
                // 🔴 Case more than one proposals has the same probability,
                // return all proposals.
                //
                trace!(
                    code = TelemetryCode::UCPLACE0019.to_string(),
                    "Inconclusive identity found at clade",
                );

                return Ok(Inconclusive(
                    clade_proposals
                        .iter()
                        .map(|item| AdherenceTest {
                            clade: match &item.clade {
                                UntaggedParent::Record(clade) => {
                                    UntaggedParent::Id(clade.id.to_owned())
                                }
                                id => id.to_owned(),
                            },
                            ..item.to_owned()
                        })
                        .collect(),
                    "Multiple proposals".to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::dtos::{clade::NodeType, kmers_map::KmersMap},
        synth,
        use_cases::map_kmers_to_tree,
    };
    use tempfile::TempDir;
    use uuid::Uuid;

    const K: u64 = 15;
    const L: usize = 800;

    fn build_tree(
        topology: &synth::Topology,
        records: &[(String, String)],
        m: u64,
    ) -> Tree {
        let dir = TempDir::new().unwrap();
        let tree_path = dir.path().join("tree.nwk");
        let msa_path = dir.path().join("msa.fasta");
        std::fs::write(&tree_path, synth::write_newick(topology)).unwrap();
        std::fs::write(
            &msa_path,
            synth::write_fasta(records, 60, false, false),
        )
        .unwrap();
        map_kmers_to_tree(tree_path, msa_path, Some(K), Some(m), None).unwrap()
    }

    fn default_dataset(seed: u64) -> (Tree, Vec<(String, String)>) {
        let topology = synth::default_topology();
        let records = synth::evolve(&topology, L, seed);
        (build_tree(&topology, &records, 4), records)
    }

    fn place(
        tree: &Tree,
        seq: &str,
        max_iterations: Option<i32>,
        coverage: Option<f64>,
        remove_intersection: Option<bool>,
    ) -> Result<PlacementStatus, MappedErrors> {
        place_sequence(
            &SequenceHeader::new("query"),
            &SequenceBody::new(seq),
            tree,
            &max_iterations,
            &coverage,
            &remove_intersection,
            &None,
        )
    }

    fn leaf_parent(tree: &Tree, name: &str) -> u64 {
        tree.root
            .get_leaves_with_paths(None)
            .into_iter()
            .find(|(clade, _)| clade.name.as_deref() == Some(name))
            .unwrap()
            .0
            .parent
            .unwrap()
    }

    fn identity_clade_id(status: &PlacementStatus) -> Option<u64> {
        match status {
            IdentityFound(test) => match &test.clade {
                UntaggedParent::Record(clade) => Some(clade.id),
                UntaggedParent::Id(id) => Some(*id),
            },
            _ => None,
        }
    }

    fn reverse_complement(seq: &str) -> String {
        seq.chars()
            .rev()
            .map(|c| match c {
                'A' => 'T',
                'T' => 'A',
                'C' => 'G',
                _ => 'C',
            })
            .collect()
    }

    // ? -----------------------------------------------------------------------
    // ? Hand-built trees: exact control over which node each k-mer maps to
    // ? -----------------------------------------------------------------------

    fn clade(id: u64, parent: Option<u64>, children: Vec<Clade>) -> Clade {
        let kind = match (parent, children.is_empty()) {
            (None, _) => NodeType::Root,
            (_, true) => NodeType::Leaf,
            (_, false) => NodeType::Node,
        };
        Clade {
            id,
            parent,
            kind,
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

    /// Internal node `id` under `parent` with two leaves `id+1` and `id+2`.
    fn cherry(id: u64, parent: u64) -> Clade {
        clade(
            id,
            Some(parent),
            vec![
                clade(id + 1, Some(id), vec![]),
                clade(id + 2, Some(id), vec![]),
            ],
        )
    }

    /// Indexes both strands of every segment's k-mers under its node set.
    fn hand_tree(root: Clade, segments: &[(&str, &[u64])]) -> Tree {
        let mut map = KmersMap::new(K, 4);
        for (seq, nodes) in segments {
            for (kmer, hash) in
                map.build_kmer_from_string(seq.to_string(), None)
            {
                map.insert_or_append_kmer_hash(
                    kmer,
                    hash,
                    nodes.iter().copied().collect(),
                );
            }
        }
        let mut tree = Tree::new(Uuid::nil(), "hand".to_string(), 0.0, root);
        tree.kmers_map = Some(map);
        tree
    }

    fn distinct_kmers(seq: &str) -> usize {
        KmersMap::new(K, 4)
            .build_kmer_from_string(seq.to_string(), None)
            .into_iter()
            .map(|(_, hash)| hash)
            .collect::<HashSet<_>>()
            .len()
    }

    /// root(0) -> P(1) -> {X(2): leaves 3,4; Y(5): leaves 6,7}; root -> Z(8).
    fn nested_root() -> Clade {
        clade(
            0,
            None,
            vec![
                clade(1, Some(0), vec![cherry(2, 1), cherry(5, 1)]),
                cherry(8, 0),
            ],
        )
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-01
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_query_shorter_than_k_returns_err_ucplace0005() {
        let (tree, records) = default_dataset(1);
        let query = &records[0].1[..K as usize - 1];

        let err = place(&tree, query, None, None, None).unwrap_err();

        assert!(err.has_str_code("UCPLACE0005"), "{err}");
    }

    #[test]
    fn place_query_of_exactly_k_has_two_kmers_and_is_not_rejected() {
        let (tree, records) = default_dataset(1);
        let query = &records[0].1[..K as usize];

        match place(&tree, query, None, None, None) {
            Err(err) => assert!(!err.has_str_code("UCPLACE0005"), "{err}"),
            Ok(_) => (),
        }
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-02
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_unrelated_query_returns_unclassifiable() {
        let (tree, _) = default_dataset(1);

        let status =
            place(&tree, &synth::random_seq(L, 999), None, None, None).unwrap();

        assert!(matches!(status, Unclassifiable(_)), "{status:?}");
    }

    #[test]
    fn place_query_with_kmers_not_mapped_to_root_returns_unclassifiable() {
        let query = synth::random_seq(200, 3);
        let tree = hand_tree(nested_root(), &[(&query, &[2])]);

        let status = place(&tree, &query, None, None, None).unwrap();

        assert!(
            matches!(&status, Unclassifiable(msg) if msg.contains("no overlapping kmers")),
            "{status:?}"
        );
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-03
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_mutated_leaf_returns_identity_found_at_its_clade() {
        let (tree, records) = default_dataset(1);

        for (i, (name, seq)) in records.iter().enumerate() {
            let query = synth::mutate(seq, 10, 100 + i as u64);
            let status = place(&tree, &query, None, None, None).unwrap();

            assert_eq!(
                identity_clade_id(&status),
                Some(leaf_parent(&tree, name)),
                "{name}: {status:?}"
            );
        }
    }

    #[test]
    fn place_reverse_complement_fragment_returns_identity_found_at_its_clade() {
        let (tree, records) = default_dataset(2);
        let (name, seq) = &records[5];
        let query = reverse_complement(&synth::fragment(seq, 200, 250));

        let status = place(&tree, &query, None, None, None).unwrap();

        assert_eq!(identity_clade_id(&status), Some(leaf_parent(&tree, name)));
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-04
    // ? -----------------------------------------------------------------------

    fn tied_tree() -> (Tree, String) {
        let query = synth::random_seq(200, 4);
        let tree = hand_tree(
            nested_root(),
            &[(&query[..100], &[0, 1, 2]), (&query[100..], &[0, 1, 5])],
        );
        (tree, query)
    }

    #[test]
    #[ignore = "BUG-020: Inconclusive is unreachable; one-vs-rest union allows at most one proposal"]
    fn place_equidistant_query_returns_inconclusive_with_both_clades() {
        let (tree, query) = tied_tree();

        let status = place(&tree, &query, None, None, None).unwrap();

        match status {
            Inconclusive(tests, _) => {
                let ids: HashSet<u64> = tests
                    .iter()
                    .map(|t| match &t.clade {
                        UntaggedParent::Id(id) => *id,
                        UntaggedParent::Record(c) => c.id,
                    })
                    .collect();
                assert_eq!(ids, HashSet::from([2, 5]));
            }
            other => panic!("expected Inconclusive, got {other:?}"),
        }
    }

    #[test]
    fn place_equidistant_query_current_behavior() {
        let (tree, query) = tied_tree();

        for remove_intersection in [false, true] {
            let status =
                place(&tree, &query, None, None, Some(remove_intersection))
                    .unwrap();

            assert_eq!(
                status,
                MaxResolutionReached(1, "LCA Accepted".to_string()),
                "remove_intersection={remove_intersection}"
            );
        }
    }

    #[test]
    fn place_equidistant_query_at_root_level_current_behavior() {
        let query = synth::random_seq(200, 5);
        let root = clade(0, None, vec![cherry(1, 0), cherry(4, 0)]);
        let tree = hand_tree(
            root,
            &[(&query[..100], &[0, 1]), (&query[100..], &[0, 4])],
        );

        let status = place(&tree, &query, None, None, None).unwrap();

        assert!(
            matches!(&status, Unclassifiable(msg) if msg.contains("Tree introspection not possible")),
            "{status:?}"
        );
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-05
    // ? -----------------------------------------------------------------------

    /// `root_fraction` of the query k-mers map to root and X(1); the rest map
    /// to X(1) only.
    fn partial_root_tree(root_len: usize) -> (Tree, String) {
        let query = synth::random_seq(400, 6);
        let root = clade(0, None, vec![cherry(1, 0)]);
        let tree = hand_tree(
            root,
            &[(&query[..root_len], &[0, 1]), (&query[root_len..], &[1])],
        );
        (tree, query)
    }

    #[test]
    fn place_coverage_below_default_threshold_returns_unclassifiable() {
        let (tree, query) = partial_root_tree(240);

        let status = place(&tree, &query, None, None, None).unwrap();

        assert!(
            matches!(&status, Unclassifiable(msg) if msg.starts_with("Insufficient kmers coverage")),
            "{status:?}"
        );
    }

    #[test]
    fn place_coverage_above_default_threshold_is_placed() {
        let (tree, query) = partial_root_tree(320);

        let status = place(&tree, &query, None, None, None).unwrap();

        assert_eq!(identity_clade_id(&status), Some(1), "{status:?}");
    }

    #[test]
    fn place_coverage_threshold_is_configurable() {
        let (tree, query) = partial_root_tree(240);

        let status = place(&tree, &query, None, Some(0.5), None).unwrap();

        assert_eq!(identity_clade_id(&status), Some(1), "{status:?}");
    }

    #[test]
    fn place_coverage_is_clamped_to_unit_interval() {
        let (tree, records) = default_dataset(1);
        let expected = Some(leaf_parent(&tree, &records[0].0));

        for coverage in [5.0, -3.0] {
            let status =
                place(&tree, &records[0].1, None, Some(coverage), None)
                    .unwrap();
            assert_eq!(identity_clade_id(&status), expected, "{coverage}");
        }

        let (tree, query) = partial_root_tree(240);
        let status = place(&tree, &query, None, Some(-3.0), None).unwrap();
        assert_eq!(identity_clade_id(&status), Some(1), "{status:?}");
    }

    #[test]
    fn place_query_with_half_unrelated_kmers_is_below_coverage() {
        let (tree, records) = default_dataset(1);
        let query = format!(
            "{}{}",
            &records[0].1[..L / 2],
            synth::random_seq(L / 2, 77)
        );

        let status = place(&tree, &query, None, None, None).unwrap();

        assert!(matches!(status, Unclassifiable(_)), "{status:?}");
    }

    #[test]
    fn place_query_with_half_unrelated_kmers_is_placed_below_its_coverage() {
        let (tree, records) = default_dataset(1);
        let query = format!(
            "{}{}",
            &records[0].1[..L / 2],
            synth::random_seq(L / 2, 77)
        );

        let status = place(&tree, &query, None, Some(0.45), None).unwrap();

        assert_eq!(
            identity_clade_id(&status),
            Some(leaf_parent(&tree, &records[0].0)),
            "{status:?}"
        );
    }

    #[test]
    fn place_identical_query_passes_full_coverage() {
        let (tree, records) = default_dataset(1);

        let status =
            place(&tree, &records[0].1, None, Some(1.0), None).unwrap();

        assert_eq!(
            identity_clade_id(&status),
            Some(leaf_parent(&tree, &records[0].0)),
            "{status:?}"
        );
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-06
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_max_iterations_exhaustion_returns_err_ucplace0010() {
        let (tree, query) = tied_tree();

        for max_iterations in [0, 1] {
            let err = place(&tree, &query, Some(max_iterations), None, None)
                .unwrap_err();
            assert!(err.has_str_code("UCPLACE0010"), "{max_iterations}: {err}");
        }

        assert!(place(&tree, &query, Some(2), None, None).is_ok());
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-07
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_remove_intersection_drops_shared_kmers_from_one_and_rest() {
        let query = synth::random_seq(220, 7);
        let (a, b, c) = (&query[..120], &query[120..180], &query[180..]);
        let root = clade(0, None, vec![cherry(1, 0), cherry(4, 0)]);
        let tree =
            hand_tree(root, &[(a, &[0, 1]), (b, &[0, 1, 4]), (c, &[0, 4])]);
        let (na, nb, nc) =
            (distinct_kmers(a), distinct_kmers(b), distinct_kmers(c));

        let counts = |remove_intersection| match place(
            &tree,
            &query,
            None,
            None,
            Some(remove_intersection),
        )
        .unwrap()
        {
            IdentityFound(test) => (
                identity_clade_id(&IdentityFound(test.clone())),
                test.one,
                test.rest,
            ),
            other => panic!("{other:?}"),
        };

        assert_eq!(
            counts(false),
            (Some(1), (na + nb) as i32, (nb + nc) as i32)
        );
        assert_eq!(counts(true), (Some(1), na as i32, nc as i32));
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-08
    // ? -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "The tree does not have a kmers map.")]
    fn place_without_kmers_map_panics() {
        let (mut tree, records) = default_dataset(1);
        tree.kmers_map = None;

        let _ = place(&tree, &records[0].1, None, None, None);
    }

    #[test]
    fn place_root_without_children_returns_err() {
        let query = synth::random_seq(200, 8);
        let tree = hand_tree(clade(0, None, vec![]), &[(&query, &[0])]);

        let err = place(&tree, &query, None, None, None).unwrap_err();

        assert!(
            err.msg().contains("root node does not have children"),
            "{err}"
        );
    }

    // ? -----------------------------------------------------------------------
    // ? PLACE-09 (S-12)
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_query_identical_to_root_leaf_child_current_behavior() {
        let topology = synth::node(
            vec![
                synth::node(
                    vec![synth::leaf("A1", 0.01), synth::leaf("A2", 0.01)],
                    Some(99.0),
                    0.08,
                ),
                synth::leaf("C", 0.09),
            ],
            None,
            0.0,
        );
        let records = synth::evolve(&topology, L, 9);
        let tree = build_tree(&topology, &records, 4);
        let c_seq = &records.iter().find(|(n, _)| n == "C").unwrap().1;

        let status = place(&tree, c_seq, None, None, None).unwrap();

        // Leaf C is filtered out of the sibling set, so node (A1,A2) is the
        // only candidate, gets `rest: 0`, and wins on shared conserved k-mers.
        match &status {
            IdentityFound(test) => {
                assert_eq!(
                    identity_clade_id(&status),
                    Some(leaf_parent(&tree, "A1"))
                );
                assert!(test.one > 0);
                assert_eq!(test.rest, 0);
            }
            other => panic!("{other:?}"),
        }
    }

    // ? -----------------------------------------------------------------------
    // ? Minimizer size zero
    // ? -----------------------------------------------------------------------

    #[test]
    fn place_with_minimizer_size_zero_finds_identity() {
        let topology = synth::default_topology();
        let records = synth::evolve(&topology, L, 1);
        let tree = build_tree(&topology, &records, 0);

        let status = place(&tree, &records[0].1, None, None, None).unwrap();

        assert_eq!(
            identity_clade_id(&status),
            Some(leaf_parent(&tree, &records[0].0)),
            "{status:?}"
        );
    }
}

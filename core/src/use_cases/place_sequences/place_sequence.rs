use super::{
    _dtos::IntrospectionUpdateResponse::*,
    update_introspection_node::update_introspection_node,
};
use crate::domain::dtos::{
    adherence_test::AdherenceTest,
    clade::Clade,
    evidence_rule::{
        EvidenceRule, PlacementScores, DEFAULT_CONTAINMENT_RATIO,
    },
    kmer_index::{KmerIndex, MatchSet},
    placement_response::PlacementStatus::{self, *},
    sequence::{SequenceBody, SequenceHeader},
    telemetry_code::TelemetryCode,
    tree::Tree,
};

use mycelium_base::{
    dtos::UntaggedParent,
    utils::errors::{use_case_err, MappedErrors},
};
use std::collections::HashSet;
use tracing::{debug, debug_span, trace, trace_span, Span};

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
    index: &KmerIndex,
    max_iterations: &Option<i32>,
    min_match_coverage: &Option<f64>,
    remove_intersection: &Option<bool>,
    evidence_rule: &Option<EvidenceRule>,
    parent_span: &Option<&tracing::Span>,
) -> Result<(PlacementStatus, Option<PlacementScores>), MappedErrors> {
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

    // Without a value from the caller, a calibrated database sets the
    // threshold from its own leaves (009-evidence-placement, EVD-06): a
    // ratio of the lowest share of a leaf's k-mers that the rest of the
    // tree holds.
    let calibration = tree.calibration.as_ref();

    let containment_ratio = evidence_rule
        .map(|rule| rule.min_containment_ratio)
        .unwrap_or(DEFAULT_CONTAINMENT_RATIO);

    let min_match_coverage = if let Some(value) = min_match_coverage {
        match value.to_owned() {
            value if value > 1.0 => 1.0,
            value if value < 0.0 => 0.0,
            value => value,
        }
    } else if let Some(lowest) =
        calibration.and_then(|c| c.containment_of(tree.root.id))
    {
        containment_ratio * lowest
    } else {
        0.7
    };

    // ? -----------------------------------------------------------------------
    // ? Build and validate query kmers
    // ? -----------------------------------------------------------------------

    let time = std::time::Instant::now();
    let query_kmers = index.query_kmers(sequence.seq());

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
    // ? Look up the query kmers
    //
    // Each distinct query k-mer is looked up once in the index. Only the
    // posting lists of the k-mers found are kept (005-placement-performance,
    // design D4).
    //
    // ? -----------------------------------------------------------------------

    let query_distinct_kmers = query_kmers
        .iter()
        .map(|(_, hash)| *hash)
        .collect::<HashSet<u64>>()
        .len();

    // A query longer than the reference's leaves (a genome against a
    // database of genes) is measured against a typical leaf (EVD-05).
    let query_distinct_kmers = match calibration {
        Some(c) if c.mean_leaf_kmers > 0 => {
            query_distinct_kmers.min(c.mean_leaf_kmers as usize)
        }
        _ => query_distinct_kmers,
    };

    let view = index.view();

    let mut matches = match MatchSet::find(&view, &query_kmers) {
        Ok(matches) => matches,
        Err(err) => {
            return use_case_err(format!("Could not read the database: {err}"))
                .as_error()
        }
    };

    Span::current()
        .record("query.kmers.treeMatches", &Some(matches.len() as i32));

    if matches.is_empty() {
        let msg = format!(
            "Query sequence {query:?} may not be related to the phylogeny",
            query = header
        );

        debug!(code = TelemetryCode::UCPLACE0006.to_string(), msg);

        return Ok((Unclassifiable(msg), None));
    }

    trace!(
        code = TelemetryCode::UCPLACE0006.to_string(),
        "Query kmers map built successfully"
    );

    // ? -----------------------------------------------------------------------
    // ? Keep the matches indexed under the root
    //
    // The root matches are used to evaluate the adherence of the query
    // sequence to the root node.
    //
    // ? -----------------------------------------------------------------------

    let time = std::time::Instant::now();

    matches.retain_containing(tree.root.id);

    if matches.is_empty() {
        let msg = "Query sequence has no overlapping kmers with the \
            reference tree";
        trace!(code = TelemetryCode::UCPLACE0007.to_string(), msg);
        return Ok((Unclassifiable(msg.to_string()), None));
    }

    Span::current()
        .record("subject.kmers.queryMatches", &Some(matches.len() as i32));

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

    let mut children: &[Clade] = if let Some(children) = &tree.root.children {
        children
    } else {
        return use_case_err(
            "The root node does not have children. This is unexpected.",
        )
        .as_error();
    };

    let mut iteration = 0;

    // The evidence and rival of the last child the evidence rule entered.
    let mut last_evidence = (0u32, 0u32);

    // ? -----------------------------------------------------------------------
    // ? Set the initial parent
    //
    // Symbol: 🍁
    //
    // The clade object is used to store the current clade as a parent being
    // evaluated. The symbol 🍁 indicate wether this object is updated.
    //
    // ? -----------------------------------------------------------------------

    let mut parent: &Clade = &tree.root;

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

    let introspection_coverage = matches.len();

    if introspection_coverage < expected_min_clade_coverage as usize {
        let msg =
            format!("Insufficient kmers coverage: {introspection_coverage}");

        trace!(code = TelemetryCode::UCPLACE0008.to_string(), msg);

        return Ok((Unclassifiable(msg), None));
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
        // ? Evidence rule (009-evidence-placement)
        //
        // Descend only into a child that holds k-mers specific to it, and
        // clearly more of them than any sibling. Otherwise stop at the
        // current clade.
        //
        // ? -------------------------------------------------------------------
        if let Some(rule) = evidence_rule {
            let child_ids =
                children.iter().map(|child| child.id).collect::<Vec<u64>>();

            // Novelty test (EVD-06): a query that shares less with this
            // clade than the clade's own leaves share with each other sits
            // outside its crown, so it stops here.
            let lowest = calibration
                .and_then(|c| c.containment_of(parent.id))
                .filter(|_| iteration > 1);

            if let Some(lowest) = lowest {
                let coverage =
                    matches.len() as f64 / query_distinct_kmers as f64;

                if coverage < rule.min_containment_ratio * lowest {
                    return Ok((
                        MaxResolutionReached(
                            parent.id,
                            "Query more distant than the clade's leaves"
                                .to_string(),
                        ),
                        Some(PlacementScores {
                            coverage,
                            evidence: last_evidence.0,
                            rival: last_evidence.1,
                        }),
                    ));
                }
            }

            let evidence = matches.evidence(&child_ids);

            let (chosen, best) = evidence
                .iter()
                .copied()
                .enumerate()
                .max_by_key(|(_, value)| *value)
                .unwrap_or((0, 0));

            let rival = evidence
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != chosen)
                .map(|(_, value)| *value)
                .max()
                .unwrap_or(0);

            trace!(
                code = TelemetryCode::UCPLACE0013.to_string(),
                "Clade {id}: evidence {best} vs strongest sibling {rival}",
                id = children.get(chosen).map(|c| c.id).unwrap_or(parent.id),
            );

            let decisive = best >= rule.min_evidence as usize
                && best > 0
                && rival as f64 <= rule.max_rival_ratio * best as f64;

            if !decisive {
                if iteration == 1 {
                    let msg = format!(
                        "No decisive evidence among the root children: \
                        best {best}, strongest sibling {rival}"
                    );
                    trace!(code = TelemetryCode::UCPLACE0019.to_string(), msg);
                    return Ok((Unclassifiable(msg), None));
                }

                return Ok((
                    MaxResolutionReached(
                        parent.id,
                        "Evidence below threshold".to_string(),
                    ),
                    Some(PlacementScores {
                        coverage: matches.len() as f64
                            / query_distinct_kmers as f64,
                        evidence: last_evidence.0,
                        rival: last_evidence.1,
                    }),
                ));
            }

            matches.descend(&child_ids, chosen);
            last_evidence = (best as u32, rival as u32);

            (parent, children) = match update_introspection_node(
                &children[chosen],
                best as i32,
                rival as i32,
            ) {
                Return(status) => {
                    return Ok((
                        status,
                        Some(PlacementScores {
                            coverage: matches.len() as f64
                                / query_distinct_kmers as f64,
                            evidence: last_evidence.0,
                            rival: last_evidence.1,
                        }),
                    ))
                }
                Continue(new_parent, new_children) => {
                    (new_parent, new_children)
                }
            };

            continue;
        }

        // ? -------------------------------------------------------------------
        // ? PHASE 1: Score the children of the current clade
        //
        // Every child competes, leaves included. A child's score is the
        // number of query k-mers indexed under it or, with
        // `remove_intersection`, only those not indexed under any sibling.
        // Each child is then compared with its strongest sibling
        // (003-placement-algorithm).
        //
        // ? -------------------------------------------------------------------
        let adherence_tests = {
            let scoring_time = std::time::Instant::now();

            let child_ids =
                children.iter().map(|child| child.id).collect::<Vec<u64>>();

            let scores = matches.scores(&child_ids, remove_intersection);

            let adherence_tests = children
                .iter()
                .enumerate()
                .map(|(index, clade)| {
                    let strongest_sibling = scores
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != index)
                        .map(|(_, score)| *score)
                        .max()
                        .unwrap_or(0);

                    trace!(
                        code = TelemetryCode::UCPLACE0013.to_string(),
                        "Clade {id}: one {one} vs strongest sibling {rest}",
                        id = clade.id,
                        one = scores[index],
                        rest = strongest_sibling,
                    );

                    (clade, scores[index] as i32, strongest_sibling as i32)
                })
                .collect::<Vec<(&Clade, i32, i32)>>();

            trace!(
                code = TelemetryCode::UCPLACE0012.to_string(),
                "Level clades scored (runtime {time:?}): {count}",
                time = scoring_time.elapsed(),
                count = adherence_tests.len()
            );

            adherence_tests
        };

        // ? -------------------------------------------------------------------
        // ? PHASE 2: Keep the children with the top score
        // ? -------------------------------------------------------------------
        let top = adherence_tests.iter().map(|test| test.1).max().unwrap_or(0);

        let winners = adherence_tests
            .into_iter()
            .filter(|test| top > 0 && test.1 == top)
            .collect::<Vec<(&Clade, i32, i32)>>();

        trace!(
            code = TelemetryCode::UCPLACE0014.to_string(),
            "Available proposals: {proposals}",
            proposals = winners.len()
        );

        // ? -------------------------------------------------------------------
        // ? PHASE 3: Decide
        // ? -------------------------------------------------------------------

        //
        // 🔴 No child carries evidence: stop at the current clade.
        //
        if winners.is_empty() {
            if iteration == 1 {
                let msg = "Tree introspection not possible. Query \
                    sequence has no overlapping kmers with the reference \
                    tree";
                trace!(code = TelemetryCode::UCPLACE0011.to_string(), msg);
                return Ok((Unclassifiable(msg.to_string()), None));
            }

            trace!(
                code = TelemetryCode::UCPLACE0015.to_string(),
                "No proposals found. Max resolution reached at clade {clade_id}",
                clade_id = parent.id
            );

            return Ok((
                MaxResolutionReached(parent.id, "LCA Accepted".to_string()),
                None,
            ));
        }

        //
        // 🟢 or ✅ A unique best child: descend into it.
        //
        if let [(clade, one, rest)] = winners[..] {
            //
            // 🍁 clade update
            // 🌿 children update
            //
            (parent, children) =
                match update_introspection_node(clade, one, rest) {
                    Return(status) => return Ok((status, None)),
                    Continue(new_parent, new_children) => {
                        (new_parent, new_children)
                    }
                };

            continue;
        }

        let tied_ids = winners
            .iter()
            .map(|(clade, _, _)| clade.id)
            .collect::<Vec<u64>>();

        trace!(
            code = TelemetryCode::UCPLACE0018.to_string(),
            "Multiple proposals found: {tied_ids:?}"
        );

        //
        // 🔴 A tie among the root's children: the query cannot be placed.
        //
        if iteration == 1 {
            let msg = format!(
                "Tie between root children {tied_ids:?} with {top} matching \
                kmers each"
            );
            trace!(code = TelemetryCode::UCPLACE0019.to_string(), msg);
            return Ok((Unclassifiable(msg), None));
        }

        //
        // 🔴 A tie below the root: report every tied clade.
        //
        trace!(
            code = TelemetryCode::UCPLACE0019.to_string(),
            "Inconclusive identity among clades {tied_ids:?} under clade {parent}",
            parent = parent.id
        );

        return Ok((
            Inconclusive(
                winners
                    .into_iter()
                    .map(|(clade, one, rest)| AdherenceTest {
                        clade: UntaggedParent::Id(clade.id),
                        one,
                        rest,
                    })
                    .collect(),
                "Multiple proposals".to_string(),
            ),
            None,
        ));
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
        let index = KmerIndex::freeze(tree.kmers_map.as_ref().unwrap());

        place_sequence(
            &SequenceHeader::new("query"),
            &SequenceBody::new(seq),
            tree,
            &index,
            &max_iterations,
            &coverage,
            &remove_intersection,
            &None,
            &None,
        )
        .map(|(status, _)| status)
    }

    /// EVD-06: under the evidence rule, a query that shares less with a
    /// clade than the clade's calibrated containment allows stops at that
    /// clade instead of entering its children.
    #[test]
    fn evidence_rule_stops_at_a_clade_the_query_is_too_distant_from() {
        let (mut tree, records) = default_dataset(1);
        let (name, seq) = &records[0];
        let query = synth::mutate(seq, 40, 5);
        let index = KmerIndex::freeze(tree.kmers_map.as_ref().unwrap());

        let (leaf, path) = tree
            .root
            .get_leaves_with_paths(None)
            .into_iter()
            .find(|(clade, _)| clade.name.as_deref() == Some(name.as_str()))
            .unwrap();
        let parent = path[path.len() - 2];

        // Only the leaf's parent clade demands a containment, the highest.
        let calibration = tree.calibration.as_mut().unwrap();
        calibration.containment.values_mut().for_each(|v| *v = 0);
        calibration.containment.insert(parent, 1_000_000);

        let mut place = |ratio: f64| {
            place_sequence(
                &SequenceHeader::new("query"),
                &SequenceBody::new(query.as_str()),
                &tree,
                &index,
                &None,
                &Some(0.05),
                &None,
                &Some(EvidenceRule {
                    min_containment_ratio: ratio,
                    ..EvidenceRule::default()
                }),
                &None,
            )
            .unwrap()
        };

        let (status, scores) = place(0.9);
        assert!(scores.unwrap().coverage < 0.9);
        assert_eq!(
            status,
            MaxResolutionReached(
                parent,
                "Query more distant than the clade's leaves".to_string()
            )
        );

        let (status, _) = place(0.0);
        assert_eq!(identity_clade_id(&status), Some(leaf.id));
    }

    fn leaf_id(tree: &Tree, name: &str) -> u64 {
        tree.root
            .get_leaves_with_paths(None)
            .into_iter()
            .find(|(clade, _)| clade.name.as_deref() == Some(name))
            .unwrap()
            .0
            .id
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
                Some(leaf_id(&tree, name)),
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

        assert_eq!(identity_clade_id(&status), Some(leaf_id(&tree, name)));
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

    /// BUG-020: a tie between sibling clades is reported, not hidden.
    #[test]
    fn place_equidistant_query_returns_inconclusive_with_both_clades() {
        let (tree, query) = tied_tree();

        for remove_intersection in [false, true] {
            let status =
                place(&tree, &query, None, None, Some(remove_intersection))
                    .unwrap();

            match status {
                Inconclusive(tests, msg) => {
                    let ids: HashSet<u64> = tests
                        .iter()
                        .map(|t| match &t.clade {
                            UntaggedParent::Id(id) => *id,
                            UntaggedParent::Record(c) => c.id,
                        })
                        .collect();
                    assert_eq!(ids, HashSet::from([2, 5]));
                    assert!(tests.iter().all(|t| t.one == t.rest));
                    assert_eq!(msg, "Multiple proposals");
                }
                other => panic!(
                    "remove_intersection={remove_intersection}: expected \
                    Inconclusive, got {other:?}"
                ),
            }
        }
    }

    /// A tie among the root's children is `Unclassifiable` (user decision,
    /// 003-placement-algorithm).
    #[test]
    fn place_equidistant_query_at_root_returns_unclassifiable_tie() {
        let query = synth::random_seq(200, 5);
        let root = clade(0, None, vec![cherry(1, 0), cherry(4, 0)]);
        let tree = hand_tree(
            root,
            &[(&query[..100], &[0, 1]), (&query[100..], &[0, 4])],
        );

        let status = place(&tree, &query, None, None, None).unwrap();

        assert!(
            matches!(&status, Unclassifiable(msg) if msg.starts_with("Tie between root children")),
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
            &[
                (&query[..root_len], &[0, 1, 2]),
                (&query[root_len..], &[1, 2]),
            ],
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

        assert_eq!(identity_clade_id(&status), Some(2), "{status:?}");
    }

    #[test]
    fn place_coverage_threshold_is_configurable() {
        let (tree, query) = partial_root_tree(240);

        let status = place(&tree, &query, None, Some(0.5), None).unwrap();

        assert_eq!(identity_clade_id(&status), Some(2), "{status:?}");
    }

    #[test]
    fn place_coverage_is_clamped_to_unit_interval() {
        let (tree, records) = default_dataset(1);
        let expected = Some(leaf_id(&tree, &records[0].0));

        for coverage in [5.0, -3.0] {
            let status =
                place(&tree, &records[0].1, None, Some(coverage), None)
                    .unwrap();
            assert_eq!(identity_clade_id(&status), expected, "{coverage}");
        }

        let (tree, query) = partial_root_tree(240);
        let status = place(&tree, &query, None, Some(-3.0), None).unwrap();
        assert_eq!(identity_clade_id(&status), Some(2), "{status:?}");
    }

    #[test]
    fn place_query_with_half_unrelated_kmers_is_below_coverage() {
        let (mut tree, records) = default_dataset(1);
        let query = format!(
            "{}{}",
            &records[0].1[..L / 2],
            synth::random_seq(L / 2, 77)
        );

        // EVD-06: a calibrated database sets the default coverage from its
        // own leaves, half the lowest share a leaf has with the rest.
        let lowest = tree
            .calibration
            .as_ref()
            .and_then(|c| c.containment_of(tree.root.id))
            .unwrap();
        assert!(lowest > 0.0 && lowest < 0.9, "{lowest}");

        let status = place(&tree, &query, None, None, None).unwrap();
        assert!(!matches!(status, Unclassifiable(_)), "{status:?}");

        // A database without calibration keeps the 0.7 default.
        tree.calibration = None;
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
            Some(leaf_id(&tree, &records[0].0)),
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
            Some(leaf_id(&tree, &records[0].0)),
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
        let root = clade(
            0,
            None,
            vec![clade(1, Some(0), vec![]), clade(4, Some(0), vec![])],
        );
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

    /// BUG-027: a leaf next to an internal clade is a placement candidate.
    #[test]
    fn place_query_identical_to_leaf_next_to_clade_is_placed_at_that_leaf() {
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

        match &status {
            IdentityFound(test) => {
                assert_eq!(
                    identity_clade_id(&status),
                    Some(leaf_id(&tree, "C"))
                );
                assert!(test.rest > 0, "(A1,A2) shares conserved k-mers");
                assert!(test.one > test.rest);
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
            Some(leaf_id(&tree, &records[0].0)),
            "{status:?}"
        );
    }
}

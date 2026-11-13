use super::{
    clade::Clade,
    kmer_index::query_kmers,
    kmers_map::{KmersMap, MinimizerKey},
};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The window lengths, in bases, at which the root crossing is measured.
const CROSSING_WINDOWS: [usize; 5] = [100, 150, 250, 500, 1000];

/// The most windows of one length taken from a leaf, evenly spaced.
const CROSSING_WINDOWS_PER_LEAF: usize = 32;

/// What the reference says about itself, measured at `build-db`
/// (009-evidence-placement, EVD-05 and EVD-06).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    /// The mean number of distinct k-mers of a leaf. A query with more
    /// k-mers than this (a genome against a database of genes) has its
    /// coverage measured against this number instead of its own.
    pub mean_leaf_kmers: u64,

    /// Per internal clade: the lowest share of a leaf's k-mers that the rest
    /// of the clade also holds, over the clade's leaves, in parts per
    /// million (an integer, so the database file round-trips exactly). A
    /// query that shares less than this with the clade is more distant from
    /// it than any of its own leaves.
    pub containment: BTreeMap<u64, u32>,

    /// Per sequence length in bases: the median share of a leaf's k-mers
    /// that leaves under another child of the root hold, in parts per
    /// million (014-root-crossing). The lengths are those of
    /// `CROSSING_WINDOWS`, measured on windows of the leaves, and the mean
    /// leaf length, measured on whole leaves. A query that shares no more
    /// than this with the reference is no closer to it than the two sides
    /// of its root are to each other.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub root_crossing: BTreeMap<u32, u32>,
}

impl Calibration {
    /// The root crossing for a query of `length` bases: the one measured at
    /// the nearest length, by ratio.
    pub fn root_crossing_of(&self, length: usize) -> Option<f64> {
        let length = (length.max(1) as f64).ln();

        self.root_crossing
            .iter()
            .min_by(|a, b| {
                let a = ((*a.0).max(1) as f64).ln() - length;
                let b = ((*b.0).max(1) as f64).ln() - length;
                a.abs().total_cmp(&b.abs())
            })
            .map(|(_, ppm)| *ppm as f64 / 1e6)
    }

    /// The lowest share of its k-mers that a query of `length` bases must
    /// find in the reference to be placed, without a value from the caller.
    ///
    /// The floor is `ratio` of the root containment (EVD-06). Above it, the
    /// query must reach the root containment or the root crossing, whichever
    /// is lower: it must look as related to the reference as its least
    /// related leaf, or more related than the two sides of the root are to
    /// each other.
    pub fn root_coverage(
        &self,
        root: u64,
        ratio: f64,
        length: usize,
    ) -> Option<f64> {
        let lowest = self.containment_of(root)?;
        let floor = ratio * lowest;

        Some(match self.root_crossing_of(length) {
            Some(crossing) => floor.max(lowest.min(crossing)),
            None => floor,
        })
    }

    /// Measure `root_crossing` from the reference sequences, given by leaf
    /// name. Sequences that match no leaf are skipped.
    pub fn measure_root_crossing(
        &mut self,
        root: &Clade,
        map: &KmersMap,
        records: &[(String, String)],
    ) {
        // The child of the root each leaf sits under
        let mut side_of = HashMap::<&str, u64>::new();

        for child in root.children.as_deref().unwrap_or_default() {
            let mut stack = vec![child];

            while let Some(clade) = stack.pop() {
                match clade.children.as_deref() {
                    Some(children) if !children.is_empty() => {
                        stack.extend(children)
                    }
                    _ => {
                        if let Some(name) = clade.name.as_deref() {
                            side_of.insert(name, child.id);
                        }
                    }
                }
            }
        }

        let sides = side_of.values().copied().collect::<HashSet<u64>>();
        let (k_size, m_size) = (map.get_kmer_size(), map.get_minimizer_size());

        // The share of the distinct k-mers of `sequence` that a leaf under
        // another child of the root holds
        let crossing = |sequence: &str, side: u64| -> Option<u32> {
            let mut seen = HashSet::<u64>::new();
            let mut shared = 0u64;

            for (key, hash) in query_kmers(sequence, k_size, m_size) {
                if !seen.insert(hash) {
                    continue;
                }

                let across = map
                    .get_map()
                    .get(&MinimizerKey(key))
                    .and_then(|bucket| bucket.0.get(&hash))
                    .is_some_and(|nodes| {
                        sides.iter().any(|other| {
                            *other != side && nodes.contains(other)
                        })
                    });

                shared += across as u64;
            }

            (!seen.is_empty())
                .then(|| (shared * 1_000_000 / seen.len() as u64) as u32)
        };

        let leaves = records
            .iter()
            .filter_map(|(name, sequence)| {
                side_of.get(name.as_str()).map(|side| (sequence, *side))
            })
            .collect::<Vec<(&String, u64)>>();

        if leaves.is_empty() {
            return;
        }

        let mean_length =
            leaves.iter().map(|(s, _)| s.len()).sum::<usize>() / leaves.len();

        let mut shares = BTreeMap::<u32, Vec<u32>>::new();

        for (sequence, side) in &leaves {
            if let Some(share) = crossing(sequence, *side) {
                shares.entry(mean_length as u32).or_default().push(share);
            }

            for window in CROSSING_WINDOWS {
                if window >= mean_length || window > sequence.len() {
                    continue;
                }

                let room = sequence.len() - window;
                let count =
                    CROSSING_WINDOWS_PER_LEAF.min(room / window * 2 + 1);

                for i in 0..count {
                    let start = if count == 1 {
                        0
                    } else {
                        i * room / (count - 1)
                    };

                    if let Some(share) = sequence
                        .get(start..start + window)
                        .and_then(|w| crossing(w, *side))
                    {
                        shares.entry(window as u32).or_default().push(share);
                    }
                }
            }
        }

        self.root_crossing = shares
            .into_iter()
            .map(|(length, mut values)| {
                values.sort_unstable();
                (length, values[values.len() / 2])
            })
            .collect();
    }

    /// The containment of a clade as a share between 0 and 1.
    pub fn containment_of(&self, clade: u64) -> Option<f64> {
        self.containment.get(&clade).map(|ppm| *ppm as f64 / 1e6)
    }

    pub fn measure(root: &Clade, map: &KmersMap) -> Self {
        let mut parents = HashMap::<u64, u64>::new();
        let mut leaves = Vec::<u64>::new();
        let mut stack = vec![root];

        while let Some(clade) = stack.pop() {
            match clade.children.as_deref() {
                Some(children) if !children.is_empty() => {
                    for child in children {
                        parents.insert(child.id, clade.id);
                        stack.push(child);
                    }
                }
                _ => leaves.push(clade.id),
            }
        }

        let is_leaf = leaves
            .iter()
            .map(|leaf| (*leaf, ()))
            .collect::<HashMap<u64, ()>>();

        // Per leaf: its distinct k-mers and, per ancestor, the k-mers first
        // shared with another leaf at that ancestor.
        let mut leaf_kmers = HashMap::<u64, u64>::new();
        let mut shared_from = HashMap::<(u64, u64), u64>::new();
        let mut children_holding = HashMap::<u64, u32>::new();

        for nodes in map.get_map().values().flat_map(|value| value.0.values()) {
            children_holding.clear();

            for node in nodes {
                if let Some(parent) = parents.get(node) {
                    *children_holding.entry(*parent).or_default() += 1;
                }
            }

            for leaf in nodes.iter().filter(|node| is_leaf.contains_key(node)) {
                *leaf_kmers.entry(*leaf).or_default() += 1;

                let mut ancestor = parents.get(leaf);

                while let Some(clade) = ancestor {
                    if children_holding.get(clade).copied().unwrap_or(0) >= 2 {
                        *shared_from.entry((*leaf, *clade)).or_default() += 1;
                        break;
                    }

                    ancestor = parents.get(clade);
                }
            }
        }

        let mut containment = BTreeMap::<u64, u32>::new();

        for leaf in &leaves {
            let Some(total) = leaf_kmers.get(leaf).filter(|total| **total > 0)
            else {
                continue;
            };

            let mut shared = 0;
            let mut ancestor = parents.get(leaf);

            while let Some(clade) = ancestor {
                shared += shared_from.get(&(*leaf, *clade)).unwrap_or(&0);

                let share = (shared * 1_000_000 / *total) as u32;

                containment
                    .entry(*clade)
                    .and_modify(|lowest| *lowest = (*lowest).min(share))
                    .or_insert(share);

                ancestor = parents.get(clade);
            }
        }

        let counted = leaf_kmers.len().max(1) as u64;

        Calibration {
            mean_leaf_kmers: leaf_kmers.values().sum::<u64>() / counted,
            containment,
            root_crossing: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dtos::clade::NodeType;
    use std::collections::HashSet;

    fn clade(id: u64, parent: Option<u64>, children: Vec<Clade>) -> Clade {
        Clade {
            id,
            parent,
            kind: if children.is_empty() {
                NodeType::Leaf
            } else {
                NodeType::Node
            },
            name: None,
            support: None,
            length: None,
            children: (!children.is_empty()).then_some(children),
        }
    }

    /// `((1,2)3,4)0`. Leaf 1 holds four k-mers: one of its own, one shared
    /// with leaf 2, one shared with leaf 4 only, one shared with both.
    #[test]
    fn measure_gives_the_lowest_leave_one_out_share_per_clade() {
        let root = clade(
            0,
            None,
            vec![
                clade(
                    3,
                    Some(0),
                    vec![clade(1, Some(3), vec![]), clade(2, Some(3), vec![])],
                ),
                clade(4, Some(0), vec![]),
            ],
        );

        let mut map = KmersMap::new(3, 0);
        let mut insert = |kmer: &str, nodes: &[u64]| {
            map.insert_or_append_kmer_hash(
                kmer.to_string(),
                kmer.bytes().map(u64::from).sum(),
                HashSet::from_iter(nodes.iter().copied()),
            );
        };

        insert("AAA", &[0, 3, 1]);
        insert("AAC", &[0, 3, 1, 2]);
        insert("AAG", &[0, 3, 1, 4]);
        insert("AAT", &[0, 3, 1, 2, 4]);

        let calibration = Calibration::measure(&root, &map);

        assert_eq!(calibration.mean_leaf_kmers, 2);
        // Clade 3: leaf 1 shares 2 of 4 with leaf 2; leaf 2 shares 2 of 2.
        assert_eq!(calibration.containment_of(3), Some(0.5));
        // Root: leaf 1 shares 3 of 4 with the others; leaves 2 and 4 all.
        assert_eq!(calibration.containment_of(0), Some(0.75));
        assert_eq!(calibration.containment.len(), 2);
    }

    fn named_leaf(id: u64, parent: u64, name: &str) -> Clade {
        Clade {
            name: Some(name.to_string()),
            ..clade(id, Some(parent), vec![])
        }
    }

    /// `((a,b)3,c)0` built from sequences, as `build-db` does.
    fn crossing_of(a: &str, b: &str, c: &str) -> Calibration {
        let root = clade(
            0,
            None,
            vec![
                clade(
                    3,
                    Some(0),
                    vec![named_leaf(1, 3, "a"), named_leaf(2, 3, "b")],
                ),
                named_leaf(4, 0, "c"),
            ],
        );

        let mut map = KmersMap::new(5, 0);

        for (sequence, path) in
            [(a, vec![0, 3, 1]), (b, vec![0, 3, 2]), (c, vec![0, 4])]
        {
            for (kmer, hash) in
                map.build_kmer_from_string(sequence.to_string(), None)
            {
                map.insert_or_append_kmer_hash(
                    kmer,
                    hash,
                    HashSet::from_iter(path.iter().copied()),
                );
            }
        }

        let mut calibration = Calibration::measure(&root, &map);

        calibration.measure_root_crossing(
            &root,
            &map,
            &[("a", a), ("b", b), ("c", c)]
                .map(|(name, s)| (name.to_string(), s.to_string())),
        );

        calibration
    }

    #[test]
    fn root_crossing_is_the_share_held_across_the_root() {
        let a = "ACGTTGCAAGGCTTAACCGGATCGATTACG";
        let unrelated = "TTTTTTTTTTCCCCCCCCCCTTTTTTTTTT";

        // The same sequence on both sides of the root: all of it crosses.
        let same = crossing_of(a, a, a);
        assert_eq!(same.root_crossing_of(30), Some(1.0));

        // Nothing shared across the root, whatever `a` and `b` share.
        let apart = crossing_of(a, a, unrelated);
        assert_eq!(apart.root_crossing_of(30), Some(0.0));

        // One key, the mean leaf length: the leaves are shorter than every
        // window.
        assert_eq!(same.root_crossing.keys().collect::<Vec<_>>(), [&30]);
    }

    #[test]
    fn root_crossing_is_read_at_the_nearest_length() {
        let calibration = Calibration {
            mean_leaf_kmers: 0,
            containment: BTreeMap::new(),
            root_crossing: BTreeMap::from([
                (150, 100_000),
                (250, 200_000),
                (1900, 300_000),
            ]),
        };

        assert_eq!(calibration.root_crossing_of(90), Some(0.1));
        assert_eq!(calibration.root_crossing_of(150), Some(0.1));
        assert_eq!(calibration.root_crossing_of(200), Some(0.2));
        assert_eq!(calibration.root_crossing_of(1800), Some(0.3));
        assert_eq!(calibration.root_crossing_of(500_000), Some(0.3));
    }

    #[test]
    fn root_coverage_is_the_lower_of_containment_and_crossing_above_the_floor()
    {
        let with = |containment: u32, crossing: Option<u32>| Calibration {
            mean_leaf_kmers: 0,
            containment: BTreeMap::from([(0, containment)]),
            root_crossing: crossing
                .map(|ppm| BTreeMap::from([(150, ppm)]))
                .unwrap_or_default(),
        };

        // Without a crossing: the floor, a ratio of the containment.
        assert_eq!(with(800_000, None).root_coverage(0, 0.25, 150), Some(0.2));
        // A crossing between the floor and the containment sets the value.
        assert_eq!(
            with(800_000, Some(500_000)).root_coverage(0, 0.25, 150),
            Some(0.5)
        );
        // A crossing above the containment: the containment.
        assert_eq!(
            with(800_000, Some(900_000)).root_coverage(0, 0.25, 150),
            Some(0.8)
        );
        // A crossing below the floor: the floor.
        assert_eq!(
            with(800_000, Some(100_000)).root_coverage(0, 0.25, 150),
            Some(0.2)
        );
        // No containment for the clade: no value.
        assert_eq!(with(800_000, None).root_coverage(9, 0.25, 150), None);
    }
}

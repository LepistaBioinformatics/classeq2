use super::{clade::Clade, kmers_map::KmersMap};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

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
}

impl Calibration {
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

        for nodes in map.get_map().values().flat_map(|value| value.0.values())
        {
            children_holding.clear();

            for node in nodes {
                if let Some(parent) = parents.get(node) {
                    *children_holding.entry(*parent).or_default() += 1;
                }
            }

            for leaf in nodes.iter().filter(|node| is_leaf.contains_key(node))
            {
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
}

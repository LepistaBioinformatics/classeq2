use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap, HashSet};

/// A map from kmers to sets of node IDs.
///
/// Structure when deserialized from JSON:
///
/// ```json
/// {
///     "kSize": 8,
///     "map": {
///         "ATCGATCG": [1, 2, 3],
///         "ATGCATGC": [4, 5, 6]
///     }
/// }
/// ```
///
/// ```yaml
/// kSize: 8
/// map:
///     ATCGATCG:
///     - 1
///     - 2
///     - 3
///     ATGCATGC:
///     - 4
///     - 5
///     - 6
/// ```
///
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KmersMap {
    #[serde(rename = "kSize")]
    k_size: usize,

    #[serde(serialize_with = "ordered_map")]
    map: HashMap<String, HashSet<i32>>,
}

/// Serialize a HashMap as an ordered map.
///
/// Sort keys and values in a HashMap before serializing it.
fn ordered_map<S, K: Ord + Serialize, V: Serialize + Into<HashSet<i32>>>(
    value: &HashMap<K, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: Into<HashSet<i32>>,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}

impl KmersMap {
    /// The constructor for a new KmersMap.
    pub fn new(k_size: usize) -> Self {
        KmersMap {
            k_size,
            map: HashMap::new(),
        }
    }

    /// Get the map of kmers.
    pub fn get_map(&self) -> &HashMap<String, HashSet<i32>> {
        &self.map
    }

    /// Get the kmer size.
    pub fn get_k_size(&self) -> usize {
        self.k_size
    }

    /// Insert a kmer into the map.
    ///
    /// If the kmer is already present, the node will be added to the existing
    /// set and the function will return false. Otherwise, the kmer will be
    /// inserted and the function will return true.
    ///
    pub fn insert_or_append(
        &mut self,
        kmer: String,
        nodes: HashSet<i32>,
    ) -> bool {
        if self.map.contains_key(&kmer) {
            if let Some(set) = self.map.get_mut(&kmer) {
                set.extend(nodes);
                let mut set_as_vec: Vec<i32> =
                    set.clone().into_iter().collect();

                set_as_vec.sort();
                set.clear();
                set.extend(set_as_vec);
            }

            return false;
        }

        self.map.insert(kmer, nodes);
        true
    }

    /// Get the set of nodes associated with a kmer.
    ///
    /// Returns None if the kmer is not present in the map. This function is
    /// used to get the clades associated with a kmer when the kmer is known.
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use classeq_core::domain::dtos::kmers_map::KmersMap;
    ///
    /// let mut kmers_map = KmersMap::new();
    /// kmers_map.insert_or_append("ATCG".to_string(), HashSet::new());
    /// kmers_map.insert_or_append("ATGC".to_string(), HashSet::new());
    /// kmers_map.insert_or_append("ATCG".to_string(), [1].iter().cloned().collect());
    /// kmers_map.insert_or_append("ATGC".to_string(), [2].iter().cloned().collect());
    ///
    /// let ids = kmers_map.get_clades_with_kmer("ATCG");
    /// assert_eq!(ids, Some(&[1].iter().cloned().collect()));
    ///
    /// let ids = kmers_map.get_clades_with_kmer("ATGC");
    /// assert_eq!(ids, Some(&[2].iter().cloned().collect()));
    ///
    /// let ids = kmers_map.get_clades_with_kmer("ATTA");
    /// assert_eq!(ids, None);
    /// ```
    ///
    pub fn get_clades_with_kmer(&self, kmer: &str) -> Option<&HashSet<i32>> {
        self.map.get(kmer).to_owned()
    }

    /// Get all kmers that contain a given node.
    ///
    /// Returns an empty set if the node is not present in any kmer. This method
    /// is used to get all kmers that contain a given clade during the
    /// prediction process.
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use classeq_core::domain::dtos::kmers_map::KmersMap;
    ///
    /// let mut kmers_map = KmersMap::new();
    /// kmers_map.insert_or_append("ATCG".to_string(), HashSet::new());
    /// kmers_map.insert_or_append("ATGC".to_string(), HashSet::new());
    /// kmers_map.insert_or_append("ATCG".to_string(), [1].iter().cloned().collect());
    /// kmers_map.insert_or_append("ATGC".to_string(), [2].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_kmers_with_node(1);
    /// assert_eq!(kmers, Some(["ATCG"].into()));
    ///
    /// let kmers = kmers_map.get_kmers_with_node(2);
    /// assert_eq!(kmers, Some(["ATGC"].into()));
    ///
    /// let kmers = kmers_map.get_kmers_with_node(3);
    /// assert_eq!(kmers, None);
    /// ```
    ///
    pub fn get_kmers_with_node(&self, node: i32) -> Option<HashSet<String>> {
        match self
            .map
            .par_iter()
            .filter_map(|(kmer, nodes)| {
                if nodes.contains(&node) {
                    Some(kmer.as_str())
                } else {
                    None
                }
            })
            .collect::<HashSet<&str>>()
        {
            set if set.is_empty() => None,
            set => Some(set.iter().map(|s| s.to_string()).collect()),
        }
    }

    /// Filter map keys by a set of kmers.
    pub fn get_overlapping_kmers(&self, kmers: &HashSet<String>) -> Self {
        let mut map = Self::new(self.k_size);

        self.map
            .par_iter()
            .map(|(key, _)| key.to_string())
            .collect::<HashSet<String>>()
            .intersection(kmers)
            .for_each(|kmer: &String| {
                if let Some(nodes) = self.map.get(kmer) {
                    map.map.insert(
                        kmer.to_string(),
                        nodes.iter().cloned().collect(),
                    );
                }
            });

        map
    }

    pub fn get_kmers_intersection(
        &self,
        kmers: &HashSet<String>,
    ) -> HashSet<String> {
        self.map
            .par_iter()
            .map(|(key, _)| key.to_string())
            .collect::<HashSet<String>>()
            .intersection(kmers)
            .cloned()
            .collect()
    }

    /// Build kmers from a string
    ///
    /// Returns a vector of kmers from a given string. This method is used to
    /// build kmers from a given sequence.
    ///
    /// # Example
    ///
    /// ```
    /// use classeq_core::domain::dtos::kmers_map::KmersMap;
    ///
    /// let sequence = "ATCG".to_string();
    /// let kmers_map = KmersMap::new(0);
    ///
    /// let kmers = kmers_map.build_kmers_from_string(sequence.to_owned(), Some(1));
    /// assert_eq!(kmers, ["A", "T", "C", "G"]);
    ///
    /// let kmers = kmers_map.build_kmers_from_string(sequence.to_owned(), Some(2));
    /// assert_eq!(kmers, ["AT", "TC", "CG"]);
    ///
    /// let kmers = kmers_map.build_kmers_from_string(sequence.to_owned(), Some(3));
    /// assert_eq!(kmers, ["ATC", "TCG"]);
    ///
    /// let kmers = kmers_map.build_kmers_from_string(sequence.to_owned(), Some(4));
    /// assert_eq!(kmers, ["ATCG"]);
    ///
    /// let kmers = kmers_map.build_kmers_from_string(sequence.to_owned(), Some(5));
    /// assert_eq!(kmers, Vec::<String>::new());
    /// ```
    ///
    pub fn build_kmers_from_string(
        &self,
        sequence: String,
        k_size: Option<usize>,
    ) -> Vec<String> {
        let mut kmers = Vec::new();
        let size = k_size.unwrap_or(self.k_size);

        if sequence.len() < self.k_size {
            return vec![];
        }

        let sequence_uppercase = sequence.to_uppercase();

        let original_sequence = sequence_uppercase.as_bytes();
        for i in 0..original_sequence.len() - size + 1 {
            let kmer =
                String::from_utf8(original_sequence[i..i + size].to_vec())
                    .unwrap();
            kmers.push(kmer);
        }

        let binding = sequence_uppercase.chars().rev().collect::<String>();
        let reverse_sequence = binding.as_bytes();
        for i in 0..reverse_sequence.len() - size + 1 {
            let kmer =
                String::from_utf8(reverse_sequence[i..i + size].to_vec())
                    .unwrap();
            kmers.push(kmer);
        }

        kmers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_or_append() {
        let mut kmers_map = KmersMap::new(0);
        let kmer = "ATCG".to_string();

        let result = kmers_map.insert_or_append(kmer.clone(), HashSet::new());
        assert_eq!(result, true);

        kmers_map.insert_or_append(kmer.clone(), [2].iter().cloned().collect());
        kmers_map.insert_or_append(kmer.clone(), [1].iter().cloned().collect());

        let result = kmers_map
            .insert_or_append(kmer.clone(), [2].iter().cloned().collect());
        assert_eq!(result, false);
    }

    #[test]
    fn test_get_clades_with_kmer() {
        let mut kmers_map = KmersMap::new(0);
        kmers_map.insert_or_append("ATCG".to_string(), HashSet::new());
        kmers_map.insert_or_append("ATGC".to_string(), HashSet::new());

        kmers_map.insert_or_append(
            "ATCG".to_string(),
            [1].iter().cloned().collect(),
        );

        kmers_map.insert_or_append(
            "ATGC".to_string(),
            [2].iter().cloned().collect(),
        );

        let ids: Option<&HashSet<i32>> = kmers_map.get_clades_with_kmer("ATCG");
        assert_eq!(ids, Some(&[1].iter().cloned().collect::<HashSet<i32>>()));

        let ids = kmers_map.get_clades_with_kmer("ATGC");
        assert_eq!(ids, Some(&[2].iter().cloned().collect::<HashSet<i32>>()));

        let ids = kmers_map.get_clades_with_kmer("ATTA");
        assert_eq!(ids, None);
    }

    #[test]
    fn test_get_kmers_with_node() {
        let sequence = "ATCG".to_string();
        let kmers_map = KmersMap::new(0);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(1));
        assert_eq!(kmers, ["A", "T", "C", "G"]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(2));
        assert_eq!(kmers, ["AT", "TC", "CG"]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(3));
        assert_eq!(kmers, ["ATC", "TCG"]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(4));
        assert_eq!(kmers, ["ATCG"]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(5));
        assert_eq!(kmers, Vec::<String>::new());
    }
}

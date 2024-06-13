use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize, Serializer};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    hash::{DefaultHasher, Hasher},
};

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
    map: HashMap<u64, HashSet<i32>>,
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
    ///
    /// Returns a new KmersMap with the given kmer size.
    ///
    pub fn new(k_size: usize) -> Self {
        KmersMap {
            k_size,
            map: HashMap::new(),
        }
    }

    /// Get the map of kmers.
    ///
    /// Returns a reference to the map of kmers. This method is used to get the
    /// map of kmers.
    ///
    pub(crate) fn get_map(&self) -> &HashMap<u64, HashSet<i32>> {
        &self.map
    }

    /// Insert a kmer into the map.
    ///
    /// If the kmer is already present, the node will be added to the existing
    /// set and the function will return false. Otherwise, the kmer will be
    /// inserted and the function will return true.
    ///
    pub(crate) fn insert_or_append_kmer_hash(
        &mut self,
        kmer: u64,
        nodes: HashSet<i32>,
    ) -> bool {
        self.insert_or_append(kmer, nodes)
    }

    /// Insert a kmer into the map.
    ///
    /// If the kmer is already present, the node will be added to the existing
    /// set and the function will return false. Otherwise, the kmer will be
    /// inserted and the function will return true.
    ///
    pub(crate) fn hash_kmer(kmer: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write(kmer.as_bytes());
        hasher.finish()
    }

    /// Insert a kmer into the map.
    ///
    /// If the kmer is already present, the node will be added to the existing
    /// set and the function will return false. Otherwise, the kmer will be
    /// inserted and the function will return true.
    ///
    fn insert_or_append(&mut self, kmer: u64, nodes: HashSet<i32>) -> bool {
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
    pub(crate) fn get_kmers_with_node(
        &self,
        node: i32,
    ) -> Option<HashSet<&u64>> {
        match self
            .map
            .par_iter()
            .filter_map(|(kmer, nodes)| {
                if nodes.contains(&node) {
                    Some(kmer)
                } else {
                    None
                }
            })
            .collect::<HashSet<&u64>>()
        {
            set if set.is_empty() => None,
            set => Some(set.iter().map(|s| s.to_owned()).collect()),
        }
    }

    /// Filter map keys by a set of kmers.
    ///
    /// Returns a new KmersMap with only the kmers that are present in the given
    /// set. This method is used to filter the kmers map by a set of kmers.
    ///
    pub(crate) fn get_overlapping_kmers(&self, kmers: &HashSet<u64>) -> Self {
        let mut map = Self::new(self.k_size);

        self.map
            .par_iter()
            .map(|(key, _)| key.to_owned())
            .collect::<HashSet<u64>>()
            .intersection(kmers)
            .for_each(|kmer: &u64| {
                if let Some(nodes) = self.map.get(kmer) {
                    map.map.insert(*kmer, nodes.iter().cloned().collect());
                }
            });

        map
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
    ) -> Vec<u64> {
        let mut kmers = Vec::new();
        let size = k_size.unwrap_or(self.k_size);

        if sequence.len() < self.k_size {
            return vec![];
        }

        kmers.extend(KmersMap::build_kmers_from_sequence(
            sequence.to_owned(),
            size,
        ));

        kmers.extend(KmersMap::build_kmers_from_sequence(
            KmersMap::reverse_complement(sequence),
            size,
        ));

        kmers
    }

    /// Build kmers from a sequence
    ///
    /// Returns a vector of kmers from a given sequence. This method is used to
    /// build kmers from a given sequence.
    ///
    fn build_kmers_from_sequence(sequence: String, size: usize) -> Vec<u64> {
        let mut kmers = Vec::new();
        let binding = sequence.to_uppercase();
        let sequence = binding.as_bytes();

        for i in 0..sequence.len() - size + 1 {
            let kmer =
                String::from_utf8(sequence[i..i + size].to_vec()).unwrap();
            kmers.push(KmersMap::hash_kmer(&kmer));
        }

        kmers
    }

    /// Reverse complement a sequence
    ///
    /// Returns the reverse complement of a given sequence. This method is used
    /// to get the reverse complement of a given sequence.
    ///
    fn reverse_complement(sequence: String) -> String {
        sequence
            .chars()
            .rev()
            .map(|c| match c {
                'a' | 'A' => 'T',
                't' | 'T' => 'A',
                'c' | 'C' => 'G',
                'g' | 'G' => 'C',
                _ => c,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_or_append_kmer_hash() {
        let mut kmers_map = KmersMap::new(0);
        kmers_map.insert_or_append_kmer_hash(1, [1].iter().cloned().collect());
        kmers_map.insert_or_append_kmer_hash(2, [2].iter().cloned().collect());
        kmers_map.insert_or_append_kmer_hash(1, [3].iter().cloned().collect());

        let mut expected = HashMap::new();
        expected.insert(1, [1, 3].iter().cloned().collect());
        expected.insert(2, [2].iter().cloned().collect());

        assert_eq!(kmers_map.map, expected);
    }

    #[test]
    fn test_get_overlapping_kmers() {
        let mut kmers_map = KmersMap::new(0);
        kmers_map.insert_or_append_kmer_hash(1, [1].iter().cloned().collect());
        kmers_map.insert_or_append_kmer_hash(2, [2].iter().cloned().collect());
        kmers_map.insert_or_append_kmer_hash(3, [3].iter().cloned().collect());

        let kmers_map =
            kmers_map.get_overlapping_kmers(&[1, 2].iter().cloned().collect());

        let mut expected = HashMap::new();

        expected.insert(1, [1].iter().cloned().collect());
        expected.insert(2, [2].iter().cloned().collect());

        assert_eq!(kmers_map.map, expected);

        let kmers_map =
            kmers_map.get_overlapping_kmers(&[1, 3].iter().cloned().collect());

        let mut expected = HashMap::new();

        expected.insert(1, [1].iter().cloned().collect());

        assert_eq!(kmers_map.map, expected);

        let kmers_map =
            kmers_map.get_overlapping_kmers(&[2, 3].iter().cloned().collect());

        let expected = HashMap::new();

        assert_eq!(kmers_map.map, expected);
    }

    #[test]
    fn test_build_kmers_from_string() {
        let sequence = "ATCG".to_string();
        let kmers_map = KmersMap::new(0);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(1));
        assert_eq!(kmers, [65, 84, 67, 71]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(2));
        assert_eq!(kmers, [10922, 17255, 17224]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(3));
        assert_eq!(kmers, [27756, 17255]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(4));
        assert_eq!(kmers, [27756]);

        let kmers =
            kmers_map.build_kmers_from_string(sequence.to_owned(), Some(5));
        assert_eq!(kmers, Vec::<u64>::new());
    }
}

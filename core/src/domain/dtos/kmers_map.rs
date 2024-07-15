use mur3::murmurhash3_x64_128;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct MinimizerKey(pub u64);

impl MinimizerKey {
    fn build_minimizer_from_string(kmer: &str, size: usize) -> Self {
        let minimizer = kmer.chars().take(size).collect::<String>();
        Self(KmersMap::hash_kmer(&minimizer))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MinimizerValue(pub HashMap<u64, HashSet<i32>>);

impl MinimizerValue {
    fn new() -> Self {
        MinimizerValue(HashMap::new())
    }

    fn insert_or_append(&mut self, kmer: u64, nodes: HashSet<i32>) -> bool {
        if self.0.contains_key(&kmer) {
            if let Some(set) = self.0.get_mut(&kmer) {
                set.extend(nodes);
                let mut set_as_vec: Vec<i32> =
                    set.clone().into_iter().collect();

                set_as_vec.sort();
                set.clear();
                set.extend(set_as_vec);
            }

            return false;
        }

        self.0.insert(kmer, nodes);
        true
    }

    fn get_kmers_with_node(&self, node: i32) -> Option<HashSet<&u64>> {
        match self
            .0
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
            set => Some(set.par_iter().map(|s| s.to_owned()).collect()),
        }
    }

    fn get_overlapping_kmers(&self, kmers: &HashSet<u64>) -> Self {
        let mut map = MinimizerValue(HashMap::new());

        self.0
            .par_iter()
            .map(|(key, _)| key.to_owned())
            .collect::<HashSet<u64>>()
            .intersection(kmers)
            .for_each(|kmer: &u64| {
                if let Some(nodes) = self.get(*kmer) {
                    map.0.insert(*kmer, nodes.iter().cloned().collect());
                }
            });

        map
    }

    fn get(&self, kmer: u64) -> Option<&HashSet<i32>> {
        self.0.get(&kmer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KmersMap {
    #[serde(rename = "kSize")]
    k_size: usize,

    /// The minimizer size
    #[serde(rename = "mSize")]
    m_size: usize,

    map: HashMap<MinimizerKey, MinimizerValue>,
}

impl KmersMap {
    /// The constructor for a new KmersMap.
    ///
    /// Returns a new KmersMap with the given kmer size.
    ///
    pub fn new(k_size: usize, m_size: usize) -> Self {
        KmersMap {
            k_size,
            m_size,
            map: HashMap::new(),
        }
    }

    /// Get the map of kmers.
    ///
    /// Returns a reference to the map of kmers. This method is used to get the
    /// map of kmers.
    ///
    pub(crate) fn get_map(&self) -> &HashMap<MinimizerKey, MinimizerValue> {
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
        kmer: String,
        hash: u64,
        nodes: HashSet<i32>,
    ) -> bool {
        let key = MinimizerKey::build_minimizer_from_string(&kmer, self.m_size);
        let value = MinimizerValue::new();

        if let Some(set) = self.map.get_mut(&key) {
            return set.insert_or_append(hash, nodes);
        }

        self.map.insert(key, value);
        false
    }

    /// Insert a kmer into the map.
    ///
    /// If the kmer is already present, the node will be added to the existing
    /// set and the function will return false. Otherwise, the kmer will be
    /// inserted and the function will return true.
    ///
    fn hash_kmer(kmer: &str) -> u64 {
        murmurhash3_x64_128(kmer.as_bytes(), 0).0
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
            .filter_map(|(_, value)| value.get_kmers_with_node(node))
            .flatten()
            .collect::<HashSet<&u64>>()
        {
            set if set.is_empty() => None,
            set => Some(set.par_iter().map(|s| s.to_owned()).collect()),
        }
    }

    /// Filter map keys by a set of kmers.
    ///
    /// Returns a new KmersMap with only the kmers that are present in the given
    /// set. This method is used to filter the kmers map by a set of kmers.
    ///
    pub(crate) fn get_overlapping_hashes(
        &mut self,
        kmers: &HashSet<u64>,
    ) -> Self {
        let mut map = Self::new(self.k_size, self.m_size);
        map.map = self
            .map
            .par_iter()
            .filter_map(|(key, value)| {
                let key = key.0;
                let value = value.get_overlapping_kmers(kmers);

                if value.0.is_empty() {
                    None
                } else {
                    Some((key, value))
                }
            })
            .map(|(key, value)| {
                let key = MinimizerKey(key);
                let value = value;
                (key, value)
            })
            .collect();

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
    pub fn build_kmer_from_string(
        &self,
        sequence: String,
        k_size: Option<usize>,
    ) -> Vec<(String, u64)> {
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
    fn build_kmers_from_sequence(
        sequence: String,
        size: usize,
    ) -> Vec<(String, u64)> {
        let mut kmers = Vec::new();
        let binding = sequence.to_uppercase();
        let sequence = binding.as_bytes();

        for i in 0..sequence.len() - size + 1 {
            let kmer = match String::from_utf8(sequence[i..i + size].to_vec()) {
                Ok(kmer) => kmer,
                Err(_) => panic!("Invalid character in sequence"),
            };

            kmers.push((kmer.to_owned(), KmersMap::hash_kmer(&kmer)));
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
                _ => panic!("Invalid character in sequence"),
            })
            .collect()
    }
}

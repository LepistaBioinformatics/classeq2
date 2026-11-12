use mur3::murmurhash3_x64_128;
#[cfg(test)]
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct MinimizerKey(pub u64);

impl MinimizerKey {
    fn build_minimizer_from_string(kmer: &str, size: u64) -> Self {
        let minimizer = kmer.chars().take(size as usize).collect::<String>();
        Self(KmersMap::hash_kmer(&minimizer))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MinimizerValue(pub HashMap<u64, HashSet<u64>>);

impl MinimizerValue {
    fn new() -> Self {
        MinimizerValue(HashMap::new())
    }

    fn insert_or_append(&mut self, kmer: u64, nodes: HashSet<u64>) -> bool {
        if self.0.contains_key(&kmer) {
            if let Some(set) = self.0.get_mut(&kmer) {
                set.extend(nodes);
            }

            return false;
        }

        self.0.insert(kmer, nodes);
        true
    }

    #[cfg(test)]
    fn get_hashed_kmers_with_node(&self, node: u64) -> Option<HashSet<u64>> {
        match self
            .0
            .par_iter()
            .filter_map(|(kmer, nodes)| {
                if nodes.contains(&node) {
                    Some(kmer.to_owned())
                } else {
                    None
                }
            })
            .collect::<HashSet<u64>>()
        {
            set if set.is_empty() => None,
            set => Some(set.par_iter().map(|s| s.to_owned()).collect()),
        }
    }

    #[cfg(test)]
    fn get_overlapping_hashed_kmers(&self, kmers: &HashSet<u64>) -> Self {
        let mut map = MinimizerValue::new();

        self.0
            .iter()
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

    #[cfg(test)]
    fn get(&self, kmer: u64) -> Option<&HashSet<u64>> {
        self.0.get(&kmer)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KmersMap {
    #[serde(rename = "kSize")]
    k_size: u64,

    /// The minimizer size
    #[serde(rename = "mSize")]
    m_size: u64,

    map: HashMap<MinimizerKey, MinimizerValue>,
}

impl KmersMap {
    /// The constructor for a new KmersMap.
    ///
    /// Returns a new KmersMap with the given kmer size.
    ///
    pub fn new(k_size: u64, m_size: u64) -> Self {
        KmersMap {
            k_size,
            m_size,
            map: HashMap::new(),
        }
    }

    /// Rebuild a map from its parts (`KmerIndex::thaw`).
    pub(crate) fn from_parts(
        k_size: u64,
        m_size: u64,
        map: HashMap<MinimizerKey, MinimizerValue>,
    ) -> Self {
        KmersMap {
            k_size,
            m_size,
            map,
        }
    }

    /// The bucket key of a k-mer: the hash of its first `m_size` characters.
    pub(crate) fn bucket_key(&self, kmer: &str) -> u64 {
        match self.m_size {
            0 => 0,
            size => MinimizerKey::build_minimizer_from_string(kmer, size).0,
        }
    }

    /// The spaced k-mers of both strands of a sequence
    /// (010-reference-assessment, SPS-01). `pattern` is a string of `1`
    /// (the position counts) and `0` (the position is ignored), as long as
    /// the window. A window with an `N` at a counted position gives no
    /// k-mer.
    pub(crate) fn spaced_kmers(
        sequence: &str,
        pattern: &str,
    ) -> Vec<(String, u64)> {
        let counted = pattern
            .bytes()
            .enumerate()
            .filter(|(_, flag)| *flag == b'1')
            .map(|(position, _)| position)
            .collect::<Vec<usize>>();

        let mut kmers = Vec::new();

        if sequence.len() < pattern.len() || counted.is_empty() {
            return kmers;
        }

        for strand in [
            sequence.to_uppercase(),
            KmersMap::reverse_complement(sequence.to_owned()),
        ] {
            for window in strand.as_bytes().windows(pattern.len()) {
                let kmer = counted
                    .iter()
                    .map(|position| window[*position] as char)
                    .collect::<String>();

                if !kmer.contains('N') {
                    let hash = KmersMap::hash_kmer(&kmer);
                    kmers.push((kmer, hash));
                }
            }
        }

        kmers
    }

    /// Get the map of kmers.
    ///
    /// Returns a reference to the map of kmers. This method is used to get the
    /// map of kmers.
    ///
    pub fn get_map(&self) -> &HashMap<MinimizerKey, MinimizerValue> {
        &self.map
    }

    pub fn get_kmer_size(&self) -> u64 {
        self.k_size
    }

    pub fn get_minimizer_size(&self) -> u64 {
        self.m_size
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
        nodes: HashSet<u64>,
    ) -> bool {
        let key = if self.m_size == 0 {
            // If the minimizer size is 0, use zero as the key
            MinimizerKey(0)
        } else {
            // Otherwise, build the minimizer from the kmer
            MinimizerKey::build_minimizer_from_string(&kmer, self.m_size)
        };

        // If the key is already present, insert the node into the set
        if let Some(set) = self.map.get_mut(&key) {
            return set.insert_or_append(hash, nodes);
        }

        // Otherwise, create a new key and insert the node into the set
        let mut value = MinimizerValue::new();
        value.insert_or_append(hash, nodes);
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
    /// ```text
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
    #[cfg(test)]
    pub(crate) fn get_hashed_kmers_with_node(
        &self,
        node: u64,
    ) -> Option<HashSet<u64>> {
        match self
            .map
            .par_iter()
            .filter_map(|(_, value)| value.get_hashed_kmers_with_node(node))
            .flatten()
            .collect::<HashSet<u64>>()
        {
            set if set.is_empty() => None,
            set => Some(set.par_iter().map(|s| s.to_owned()).collect()),
        }
    }

    /// Get all kmers that contain a given node.
    ///
    /// Returns an empty set if the node is not present in any kmer. This method
    /// is used to get all kmers that contain a given clade during the
    /// prediction process.
    ///
    #[cfg(test)]
    pub(crate) fn get_minimized_hashes_with_node(
        &self,
        node: u64,
    ) -> Option<HashMap<&MinimizerKey, HashSet<u64>>> {
        match self
            .map
            .par_iter()
            .filter_map(|(key, value)| {
                match value.get_hashed_kmers_with_node(node) {
                    Some(set) => Some((key, set)),
                    None => None,
                }
            })
            .collect::<HashMap<&MinimizerKey, HashSet<u64>>>()
        {
            set if set.is_empty() => None,
            set => Some(set),
        }
    }

    /// Filter map keys by a set of kmers.
    ///
    /// Returns a new KmersMap with only the kmers that are present in the given
    /// set. This method is used to filter the kmers map by a set of kmers.
    ///
    /// TODO: This method is not used and should be removed.
    ///
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn get_overlapping_hashes(&self, hashes: &HashSet<u64>) -> Self {
        let mut map = Self::new(self.k_size, self.m_size);

        map.map = self
            .map
            .par_iter()
            .filter_map(|(key, value)| {
                let key = key.0;
                let value = value.get_overlapping_hashed_kmers(hashes);

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

    /// Filter overlapping kmers by a set of kmers.
    ///
    /// Returns a new KmersMap with only the kmers that are present in the given
    /// set. This method is used to filter the kmers map by a set of kmers.
    ///
    #[cfg(test)]
    pub(crate) fn get_overlapping_hashed_kmers(
        &self,
        hashed_kmers: Vec<(String, u64)>,
    ) -> Self {
        let mut map = Self::new(self.k_size, self.m_size);

        let minimizers: HashSet<MinimizerKey> = hashed_kmers
            .par_iter()
            .map(|(kmer, _)| {
                MinimizerKey::build_minimizer_from_string(kmer, self.m_size)
            })
            .collect();

        let hashes: HashSet<u64> = hashed_kmers
            .par_iter()
            .map(|(_, hash)| hash.to_owned())
            .collect();

        map.map = self
            .map
            .par_iter()
            .filter_map(|(key, value)| {
                if !minimizers.contains(key) {
                    return None;
                }

                let value = value.get_overlapping_hashed_kmers(&hashes);

                if value.0.is_empty() {
                    None
                } else {
                    Some((key, value))
                }
            })
            .map(|(key, value)| (key.to_owned(), value))
            .collect();

        map
    }

    /// Filter overlapping kmers by a set of kmers.
    ///
    /// Returns a new KmersMap with only the kmers that are present in the given
    /// set. This method is used to filter the kmers map by a set of kmers.
    ///
    #[cfg(test)]
    pub(crate) fn get_overlapping_minimized_hashes(
        &self,
        hashed_kmers: HashMap<&MinimizerKey, HashSet<u64>>,
    ) -> Self {
        let mut map = Self::new(self.k_size, self.m_size);

        map.map = self
            .map
            .par_iter()
            .filter_map(|(key, value)| {
                if let Some(hashes) = hashed_kmers.get(key) {
                    let value = value.get_overlapping_hashed_kmers(&hashes);

                    if value.0.is_empty() {
                        None
                    } else {
                        Some((key.to_owned(), value))
                    }
                } else {
                    None
                }
            })
            .map(|(key, value)| (key.to_owned(), value))
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
    /// ```text
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
        k_size: Option<u64>,
    ) -> Vec<(String, u64)> {
        let mut kmers = Vec::new();
        let size = k_size.unwrap_or(self.k_size);

        if sequence.len() < size as usize {
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
        size: u64,
    ) -> Vec<(String, u64)> {
        let mut kmers = Vec::new();
        let binding = sequence.to_uppercase();
        let sequence = binding.as_bytes();
        let size = size as usize;

        for i in 0..sequence.len() - size + 1 {
            // A window holding an `N` gives no k-mer
            if sequence[i..i + size].contains(&b'N') {
                continue;
            }

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
                'n' | 'N' => 'N',
                _ => panic!("Invalid character in sequence"),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth;
    use std::collections::{BTreeMap, BTreeSet};

    fn mur(kmer: &str) -> u64 {
        murmurhash3_x64_128(kmer.as_bytes(), 0).0
    }

    fn set(ids: &[u64]) -> HashSet<u64> {
        ids.iter().copied().collect()
    }

    /// Flattens a map into `hash -> sorted node ids` for order-free comparison.
    fn flatten(map: &KmersMap) -> BTreeMap<u64, BTreeSet<u64>> {
        let mut out = BTreeMap::new();
        for value in map.get_map().values() {
            for (hash, nodes) in value.0.iter() {
                out.insert(*hash, nodes.iter().copied().collect());
            }
        }
        out
    }

    fn keys(map: &KmersMap) -> BTreeSet<u64> {
        map.get_map().keys().map(|k| k.0).collect()
    }

    fn expected(entries: &[(&str, &[u64])]) -> BTreeMap<u64, BTreeSet<u64>> {
        entries
            .iter()
            .map(|(kmer, nodes)| (mur(kmer), nodes.iter().copied().collect()))
            .collect()
    }

    /// ACGTA and ACTTG share the minimizer `AC` when m = 2. ACGTA is
    /// inserted twice to exercise node-set merging.
    fn fixture(m: u64) -> KmersMap {
        let mut map = KmersMap::new(5, m);
        for (kmer, nodes) in [
            ("ACGTA", set(&[0, 1])),
            ("ACTTG", set(&[0, 2])),
            ("GGCAT", set(&[0, 2])),
            ("TTTTT", set(&[0, 1, 2])),
            ("ACGTA", set(&[3])),
        ] {
            map.insert_or_append_kmer_hash(kmer.to_string(), mur(kmer), nodes);
        }
        map
    }

    // ? KMER-01 ---------------------------------------------------------------

    #[test]
    fn build_kmers_from_sequence_yields_ordered_uppercase_windows_with_murmur3_hashes(
    ) {
        let kmers = KmersMap::build_kmers_from_sequence("acgTAC".into(), 3);

        let expected: Vec<(String, u64)> = ["ACG", "CGT", "GTA", "TAC"]
            .iter()
            .map(|k| (k.to_string(), mur(k)))
            .collect();

        assert_eq!(kmers, expected);
    }

    #[test]
    fn build_kmers_from_sequence_yields_len_minus_k_plus_one_windows() {
        let seq = synth::random_seq(100, 7);

        for k in [1u64, 15, 21, 100] {
            let kmers = KmersMap::build_kmers_from_sequence(seq.clone(), k);
            assert_eq!(kmers.len(), 100 - k as usize + 1, "k = {k}");
            assert_eq!(kmers[0].0, seq[..k as usize]);
            assert_eq!(kmers.last().unwrap().0, seq[100 - k as usize..]);
            assert!(kmers.iter().all(|(kmer, hash)| *hash == mur(kmer)));
        }
    }

    #[test]
    fn hash_kmer_is_murmur3_x64_128_low_64_bits_with_seed_zero() {
        assert_eq!(
            KmersMap::hash_kmer("ACGTACGTACGTACG"),
            mur("ACGTACGTACGTACG")
        );
        assert_ne!(KmersMap::hash_kmer("ACGT"), KmersMap::hash_kmer("ACGA"));
    }

    #[test]
    fn hash_of_empty_string_is_zero_so_m0_minimizer_matches_insertion_key() {
        assert_eq!(KmersMap::hash_kmer(""), 0);
        assert_eq!(
            MinimizerKey::build_minimizer_from_string("ACGTA", 0),
            MinimizerKey(0)
        );
    }

    #[test]
    fn minimizer_key_hashes_the_first_m_characters() {
        assert_eq!(
            MinimizerKey::build_minimizer_from_string("ACGTA", 2),
            MinimizerKey(mur("AC"))
        );
        assert_eq!(
            MinimizerKey::build_minimizer_from_string("ACGTA", 9),
            MinimizerKey(mur("ACGTA"))
        );
    }

    // ? KMER-02 ---------------------------------------------------------------

    #[test]
    fn reverse_complement_reverses_and_complements_both_cases() {
        assert_eq!(KmersMap::reverse_complement("ACCGTA".into()), "TACGGT");
        assert_eq!(KmersMap::reverse_complement("accgta".into()), "TACGGT");
        assert_eq!(KmersMap::reverse_complement(String::new()), "");
    }

    #[test]
    fn reverse_complement_is_an_involution_on_acgt() {
        for seed in 0..5 {
            let seq = synth::random_seq(200, seed);
            let twice = KmersMap::reverse_complement(
                KmersMap::reverse_complement(seq.clone()),
            );
            assert_eq!(twice, seq);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid character in sequence")]
    fn reverse_complement_panics_on_non_acgtn() {
        KmersMap::reverse_complement("ACG-".into());
    }

    /// BUG-045: `N` survives the reverse complement and breaks the k-mers.
    #[test]
    fn build_kmer_from_string_skips_windows_with_n() {
        assert_eq!(KmersMap::reverse_complement("ACGN".into()), "NCGT");

        let map = KmersMap::new(3, 0);
        let kmers = map
            .build_kmer_from_string("ACGTNACGT".into(), None)
            .into_iter()
            .map(|(kmer, _)| kmer)
            .collect::<Vec<String>>();

        assert_eq!(
            kmers,
            ["ACG", "CGT", "ACG", "CGT", "ACG", "CGT", "ACG", "CGT"]
        );
    }

    // ? KMER-03 ---------------------------------------------------------------

    #[test]
    fn build_kmer_from_string_returns_forward_then_reverse_complement_kmers() {
        let map = KmersMap::new(3, 0);
        let kmers = map.build_kmer_from_string("ACGTA".into(), None);

        // reverse complement of ACGTA is TACGT
        let expected: Vec<(String, u64)> =
            ["ACG", "CGT", "GTA", "TAC", "ACG", "CGT"]
                .iter()
                .map(|k| (k.to_string(), mur(k)))
                .collect();

        assert_eq!(kmers, expected);
    }

    #[test]
    fn build_kmer_from_string_with_length_equal_to_k_returns_two_kmers() {
        let map = KmersMap::new(5, 0);
        let kmers = map.build_kmer_from_string("ACGTA".into(), None);
        let names: Vec<&str> = kmers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["ACGTA", "TACGT"]);
    }

    #[test]
    fn build_kmer_from_string_returns_empty_when_shorter_than_k() {
        let map = KmersMap::new(6, 0);
        assert!(map.build_kmer_from_string("ACGTA".into(), None).is_empty());
        assert!(map.build_kmer_from_string(String::new(), None).is_empty());
    }

    #[test]
    fn build_kmer_from_string_uses_the_k_override() {
        let map = KmersMap::new(3, 0);
        let kmers = map.build_kmer_from_string("ACGTA".into(), Some(2));
        let names: Vec<&str> = kmers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["AC", "CG", "GT", "TA", "TA", "AC", "CG", "GT"]);
    }

    /// BUG-010: the length guard compared against `self.k_size` instead of
    /// the effective size, so an override longer than the sequence
    /// underflowed and panicked.
    #[test]
    fn build_kmer_from_string_returns_empty_when_override_exceeds_length() {
        let map = KmersMap::new(3, 0);
        assert!(map
            .build_kmer_from_string("ACGTA".into(), Some(10))
            .is_empty());
    }

    /// BUG-010: a shorter override was rejected by the guard on `self.k_size`.
    #[test]
    fn build_kmer_from_string_accepts_override_shorter_than_default_k() {
        let map = KmersMap::new(10, 0);
        let kmers = map.build_kmer_from_string("ACGTA".into(), Some(3));
        assert_eq!(kmers.len(), 6);
    }

    // ? KMER-04 ---------------------------------------------------------------

    #[test]
    fn insert_groups_by_minimizer_prefix_and_merges_node_sets() {
        let map = fixture(2);

        assert_eq!(
            keys(&map),
            BTreeSet::from([mur("AC"), mur("GG"), mur("TT")])
        );

        let ac = &map.get_map()[&MinimizerKey(mur("AC"))].0;
        assert_eq!(ac.len(), 2);
        assert_eq!(ac[&mur("ACGTA")], set(&[0, 1, 3]));
        assert_eq!(ac[&mur("ACTTG")], set(&[0, 2]));

        assert_eq!(
            flatten(&map),
            expected(&[
                ("ACGTA", &[0, 1, 3]),
                ("ACTTG", &[0, 2]),
                ("GGCAT", &[0, 2]),
                ("TTTTT", &[0, 1, 2]),
            ])
        );
    }

    #[test]
    fn insert_with_m0_uses_a_single_zero_key() {
        let map = fixture(0);

        assert_eq!(keys(&map), BTreeSet::from([0]));
        assert_eq!(flatten(&map), flatten(&fixture(2)));
        assert_eq!(map.get_kmer_size(), 5);
        assert_eq!(map.get_minimizer_size(), 0);
    }

    /// S-06: the doc says `true` is returned for a new k-mer, but a new
    /// k-mer under a new minimizer key returns `false`. No caller uses the
    /// return value, so this pins the current behavior only.
    #[test]
    fn insert_or_append_kmer_hash_return_value_current_behavior() {
        let mut map = KmersMap::new(5, 2);

        assert!(!map.insert_or_append_kmer_hash(
            "ACGTA".into(),
            mur("ACGTA"),
            set(&[1])
        ));
        assert!(map.insert_or_append_kmer_hash(
            "ACTTG".into(),
            mur("ACTTG"),
            set(&[1])
        ));
        assert!(!map.insert_or_append_kmer_hash(
            "ACGTA".into(),
            mur("ACGTA"),
            set(&[2])
        ));
    }

    // ? KMER-05 ---------------------------------------------------------------

    #[test]
    fn get_hashed_kmers_with_node_returns_exact_subsets() {
        for m in [0, 2] {
            let map = fixture(m);
            let hashes = |kmers: &[&str]| -> Option<HashSet<u64>> {
                Some(kmers.iter().map(|k| mur(k)).collect())
            };

            assert_eq!(
                map.get_hashed_kmers_with_node(0),
                hashes(&["ACGTA", "ACTTG", "GGCAT", "TTTTT"]),
                "m = {m}"
            );
            assert_eq!(
                map.get_hashed_kmers_with_node(1),
                hashes(&["ACGTA", "TTTTT"])
            );
            assert_eq!(
                map.get_hashed_kmers_with_node(2),
                hashes(&["ACTTG", "GGCAT", "TTTTT"])
            );
            assert_eq!(map.get_hashed_kmers_with_node(3), hashes(&["ACGTA"]));
            assert_eq!(map.get_hashed_kmers_with_node(99), None);
        }
    }

    #[test]
    fn get_minimized_hashes_with_node_groups_hashes_by_key() {
        let map = fixture(2);
        let found = map.get_minimized_hashes_with_node(2).unwrap();

        let found: BTreeMap<u64, HashSet<u64>> =
            found.into_iter().map(|(k, v)| (k.0, v)).collect();

        assert_eq!(
            found,
            BTreeMap::from([
                (mur("AC"), set(&[mur("ACTTG")])),
                (mur("GG"), set(&[mur("GGCAT")])),
                (mur("TT"), set(&[mur("TTTTT")])),
            ])
        );
        assert!(map.get_minimized_hashes_with_node(99).is_none());

        let map = fixture(0);
        let found = map.get_minimized_hashes_with_node(1).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[&MinimizerKey(0)], set(&[mur("ACGTA"), mur("TTTTT")]));
    }

    #[test]
    fn get_overlapping_hashed_kmers_returns_only_query_kmers() {
        for m in [0, 2] {
            let map = fixture(m);
            let query: Vec<(String, u64)> = ["ACGTA", "GGCAT", "CCCCC"]
                .iter()
                .map(|k| (k.to_string(), mur(k)))
                .collect();

            let overlap = map.get_overlapping_hashed_kmers(query);

            assert_eq!(
                flatten(&overlap),
                expected(&[("ACGTA", &[0, 1, 3]), ("GGCAT", &[0, 2])]),
                "m = {m}"
            );
            assert_eq!(overlap.get_kmer_size(), 5);
            assert_eq!(overlap.get_minimizer_size(), m);
        }
    }

    #[test]
    fn get_overlapping_hashed_kmers_with_query_built_from_sequence() {
        for m in [0, 3] {
            let seq = synth::random_seq(60, 11);
            let mut map = KmersMap::new(15, m);
            for (kmer, hash) in map.build_kmer_from_string(seq.clone(), None) {
                map.insert_or_append_kmer_hash(kmer, hash, set(&[0, 1]));
            }
            let all = flatten(&map);

            let query =
                map.build_kmer_from_string(seq[10..40].to_string(), None);
            let overlap = map.get_overlapping_hashed_kmers(query.clone());

            let query_hashes: BTreeSet<u64> =
                query.iter().map(|(_, h)| *h).collect();
            assert_eq!(
                flatten(&overlap).keys().copied().collect::<BTreeSet<_>>(),
                query_hashes,
                "m = {m}"
            );
            assert!(flatten(&overlap).keys().all(|h| all.contains_key(h)));
        }
    }

    #[test]
    fn get_overlapping_minimized_hashes_returns_node_submap() {
        for m in [0, 2] {
            let map = fixture(m);
            let node_hashes = map.get_minimized_hashes_with_node(1).unwrap();
            let overlap = map.get_overlapping_minimized_hashes(node_hashes);

            assert_eq!(
                flatten(&overlap),
                expected(&[("ACGTA", &[0, 1, 3]), ("TTTTT", &[0, 1, 2])]),
                "m = {m}"
            );
        }

        let map = fixture(2);
        assert!(
            flatten(&map.get_overlapping_minimized_hashes(HashMap::new()))
                .is_empty()
        );
    }

    #[test]
    fn get_overlapping_hashes_filters_by_hash_set() {
        for m in [0, 2] {
            let map = fixture(m);
            let hashes: HashSet<u64> =
                [mur("ACTTG"), mur("TTTTT"), 12345].into_iter().collect();
            let overlap = map.get_overlapping_hashes(&hashes);

            assert_eq!(
                flatten(&overlap),
                expected(&[("ACTTG", &[0, 2]), ("TTTTT", &[0, 1, 2])]),
                "m = {m}"
            );
            assert!(keys(&overlap).is_subset(&keys(&map)));
        }
    }

    // ? KMER-06 ---------------------------------------------------------------

    #[test]
    fn kmers_map_round_trips_through_yaml() {
        for m in [0, 2] {
            let map = fixture(m);
            let yaml = serde_saphyr::to_string(&map).unwrap();
            assert!(yaml.contains("kSize: 5"));
            assert!(yaml.contains(&format!("mSize: {m}")));

            let back: KmersMap = serde_saphyr::from_str(&yaml).unwrap();
            assert_eq!(back.get_kmer_size(), 5);
            assert_eq!(back.get_minimizer_size(), m);
            assert_eq!(keys(&back), keys(&map));
            assert_eq!(flatten(&back), flatten(&map));
        }
    }

    #[test]
    fn kmers_map_round_trips_through_json() {
        for m in [0, 2] {
            let map = fixture(m);
            let json = serde_json::to_value(&map).unwrap();
            assert_eq!(json["kSize"], 5);
            assert_eq!(json["mSize"], m);

            let back: KmersMap = serde_json::from_value(json).unwrap();
            assert_eq!(keys(&back), keys(&map));
            assert_eq!(flatten(&back), flatten(&map));

            let text = serde_json::to_string(&map).unwrap();
            let back: KmersMap = serde_json::from_str(&text).unwrap();
            assert_eq!(flatten(&back), flatten(&map));
        }
    }
}

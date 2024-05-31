use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A map from kmers to sets of node IDs.
///
/// Structure when deserialized from JSON:
///
/// ```json
/// {
///    "ATCGATCG": [1, 2, 3],
///    "ATGCATGC": [4, 5, 6]
/// }
/// ```
///
#[derive(Debug, Serialize, Deserialize)]
pub struct KmersMap(HashMap<String, HashSet<i32>>);

impl KmersMap {
    /// The constructor for a new KmersMap.
    pub fn new() -> Self {
        KmersMap(HashMap::new())
    }

    /// Get the map of kmers.
    pub fn get_map(&self) -> &HashMap<String, HashSet<i32>> {
        &self.0
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
        if self.0.contains_key(&kmer) {
            self.0.get_mut(&kmer).unwrap().extend(nodes);
            return false;
        }

        self.0.insert(kmer, nodes);
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
        self.0.get(kmer).to_owned()
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
    pub fn get_kmers_with_node(&self, node: i32) -> Option<HashSet<&str>> {
        match self
            .0
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
            set => Some(set),
        }
    }

    /// Get existing kmers from hashset of kmers
    ///
    /// Returns a hashset of kmers that are present in the KmersMap. This method
    /// is used to get all kmers that are present in the KmersMap.
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
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATCG".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATCG".to_string()
    /// ].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string(),
    ///     "ATTA".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATTA".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, HashSet::new());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&HashSet::new());
    /// assert_eq!(kmers, HashSet::new());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATTA".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATTA".to_string(),
    ///     "ATGC".to_string(),
    ///     "ATCG".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATTA".to_string(),
    ///     "ATGC".to_string(),
    ///     "ATCG".to_string(),
    ///     "ATTA".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    ///
    /// let kmers = kmers_map.get_existing_kmers(&[
    ///     "ATTA".to_string(),
    ///     "ATGC".to_string(),
    ///     "ATCG".to_string(),
    ///     "ATTA".to_string(),
    ///     "ATTA".to_string()
    /// ].iter().cloned().collect());
    /// assert_eq!(kmers, [
    ///     "ATCG".to_string(),
    ///     "ATGC".to_string()
    /// ].iter().cloned().collect());
    /// ```
    ///
    pub fn get_existing_kmers(
        &self,
        kmers: &HashSet<String>,
    ) -> HashSet<String> {
        kmers
            .par_iter()
            .filter(|kmer| self.0.contains_key(*kmer))
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
    ///
    /// let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 1);
    /// assert_eq!(kmers, ["A", "T", "C", "G"]);
    ///
    /// let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 2);
    /// assert_eq!(kmers, ["AT", "TC", "CG"]);
    ///
    /// let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 3);
    /// assert_eq!(kmers, ["ATC", "TCG"]);
    ///
    /// let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 4);
    /// assert_eq!(kmers, ["ATCG"]);
    ///
    /// let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 5);
    /// assert_eq!(kmers, Vec::<String>::new());
    /// ```
    ///
    pub fn build_kmers_from_string(
        sequence: String,
        size: usize,
    ) -> Vec<String> {
        let mut kmers = Vec::new();

        if sequence.len() < size {
            return vec![];
        }

        let sequence = sequence.as_bytes();

        for i in 0..sequence.len() - size + 1 {
            let kmer =
                String::from_utf8(sequence[i..i + size].to_vec()).unwrap();
            kmers.push(kmer);
        }

        kmers
    }

    pub fn remove_non_iupac_from_sequence(sequence: &str) -> String {
        sequence
            .to_uppercase()
            .chars()
            .filter(|c| match c {
                'A' | 'C' | 'G' | 'T' => true,
                _ => false,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_clades_with_kmer() {
        let mut kmers_map = KmersMap::new();
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

        let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 1);
        assert_eq!(kmers, ["A", "T", "C", "G"]);

        let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 2);
        assert_eq!(kmers, ["AT", "TC", "CG"]);

        let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 3);
        assert_eq!(kmers, ["ATC", "TCG"]);

        let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 4);
        assert_eq!(kmers, ["ATCG"]);

        let kmers = KmersMap::build_kmers_from_string(sequence.to_owned(), 5);
        assert_eq!(kmers, Vec::<String>::new());
    }
}

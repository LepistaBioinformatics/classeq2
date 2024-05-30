use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CladeKmers {
    /// The clade ID to which the k-mer's belong.
    pub clade: Uuid,

    /// A set of k-mer's.
    kmers: HashSet<i32>,
}

impl CladeKmers {
    pub fn new(clade: Uuid) -> Self {
        Self {
            clade,
            kmers: HashSet::new(),
        }
    }

    pub fn insert(&mut self, kmer: i32) -> bool {
        self.kmers.insert(kmer)
    }

    pub fn insert_many(&mut self, kmers: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        let mut inserted_kmers = Vec::new();
        let mut ignored_kmers = Vec::new();

        for kmer in kmers {
            if self.insert(kmer) {
                inserted_kmers.push(kmer);
            } else {
                ignored_kmers.push(kmer);
            }
        }

        (inserted_kmers, ignored_kmers)
    }

    pub fn contains(&self, kmer: i32) -> bool {
        self.kmers.contains(&kmer)
    }

    pub fn len(&self) -> usize {
        self.kmers.len()
    }
}

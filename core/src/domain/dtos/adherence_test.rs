use super::clade::Clade;

use mycelium_base::dtos::UntaggedParent;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdherenceTest {
    /// The unique identifier for the clade to be tested.
    pub clade: UntaggedParent<Clade, u64>,

    /// The number of kmer matches with the desired clade.
    pub one_len: i32,

    /// The length of the rest of the sequence.
    pub rest_len: i32,

    /// The average number of kmer matches with sibling clades.
    pub rest_avg: f64,

    /// The maximum number of kmer matches with sibling clades.
    pub rest_max: i32,
}

impl std::fmt::Display for AdherenceTest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AdherenceTest: {} - {} - {}",
            self.clade, self.one_len, self.rest_avg
        )
    }
}

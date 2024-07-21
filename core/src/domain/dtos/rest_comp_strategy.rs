use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum RestComparisonStrategy {
    /// The average number of kmer matches with sibling clades.
    Avg,

    /// The maximum number of kmer matches with sibling clades.
    Max,
}

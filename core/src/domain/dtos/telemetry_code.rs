use serde::Serialize;
use std::fmt::Display;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged, rename_all = "camelCase")]
pub(crate) enum TelemetryCode {
    /// The kmers were sucessfully generated from the query sequence
    PLACE0001,

    /// The query kmers were sucessfully mapped on the referemce tree
    PLACE0002,

    /// The query kmers were sucessfully identified on the tree indices
    PLACE0003,

    /// All is done to start the tree introspection
    PLACE0004,

    /// The minimum coverage was successfully calculated
    PLACE0005,

    /// A tree introspection loop is running
    PLACE0006,

    /// A given sequence has no enouth coverage to be placed
    PLACE0007,

    /// A set of clades are right now being placed
    PLACE0008,

    /// A one-vs-rest pair is being placed
    PLACE0009,

    /// The one-vs-rest pair was successfully placed and a set of clade
    /// proposals was generated
    PLACE0010,

    /// If the clade proposal set is empty, the sequence is not placed
    PLACE0011,

    /// If the clade proposal set contains just one clade, the sequence
    /// continues to the placement process
    PLACE0012,

    /// If multiple clades are proposed, the search process are evaluated
    PLACE0013,

    /// The tree introspection process is done
    PLACE0014,

    /// The placement process was inconclusive
    PLACE0015,
}

impl Display for TelemetryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

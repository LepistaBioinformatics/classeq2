use serde::Serialize;
use std::fmt::Display;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged, rename_all = "camelCase")]
pub(crate) enum TelemetryCode {
    // ? -----------------------------------------------------------------------
    // ? Reserved codes to the place_sequences use case
    //
    /// Multiple sequence placement started
    ///
    //
    PLACE0001,
    /// Multiple sequence placement ended
    ///
    PLACE0002,
    //
    /// A single sequence placement started
    ///
    PLACE0003,
    //
    /// A single sequence placement ended
    ///
    PLACE0004,
    // ? -----------------------------------------------------------------------

    // ? -----------------------------------------------------------------------
    // ? Reserved codes to the place_sequence use case
    //
    /// Query kmers built successfully
    ///
    PLACE0005,
    //
    /// Query kmers map built successfully
    ///
    PLACE0006,
    //
    /// Root kmers map built successfully
    ///
    PLACE0007,
    //
    /// Starting tree introspection
    ///
    PLACE0008,
    //
    /// Expected min clade coverage calculated
    PLACE0009,
    //
    /// A introspection level was reached into the introspection loop
    ///
    PLACE0010,
    //
    /// A given clade has no enough coverage to be considered
    ///
    PLACE0011,
    //
    /// A one-vs-rest clade proposal set was built
    ///
    PLACE0012,
    //
    /// A one-vs-rest placement statistics was calculated
    ///
    PLACE0013,
    //
    /// A set of proposals are right to be be evaluated
    ///
    PLACE0014,
    //
    /// If empty clade proposal set was reached the `MaxResolutionReached` state
    /// is triggered
    ///
    PLACE0015,
    //
    /// If the clade proposal set contains just one clade, and a identity is
    /// reached
    ///
    PLACE0016,
    //
    /// If the clade proposal set contains just one clade, and the clade
    /// contains children clades, go to the next level
    ///
    PLACE0017,
    //
    /// Multiple clade proposal set was reached
    ///
    PLACE0018,
    //
    /// If multiple clade proposal set was reached, and the algorithm was unable
    /// to filter proposals to a conclusive one, the `Inconclusive` state is
    /// triggered
    ///
    PLACE0019,
    // ? -----------------------------------------------------------------------
}

impl Display for TelemetryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

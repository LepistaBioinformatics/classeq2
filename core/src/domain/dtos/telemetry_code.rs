use serde::Serialize;
use std::fmt::Display;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged, rename_all = "UPPERCASE")]
pub(crate) enum TelemetryCode {
    // ? -----------------------------------------------------------------------
    // ? Reserved codes to the place_sequences use case
    //
    /// Multiple sequence placement started
    ///
    //
    UCPLACE0001,
    //
    /// Multiple sequence placement ended
    ///
    UCPLACE0002,
    //
    /// A single sequence placement started
    ///
    UCPLACE0003,
    //
    /// A single sequence placement ended
    ///
    UCPLACE0004,
    //
    /// Indicate that te model include annotations and it should be used to
    /// finish the placement process
    ///
    UCPLACE00020,
    // ? -----------------------------------------------------------------------

    // ? -----------------------------------------------------------------------
    // ? Reserved codes to the place_sequence use case
    //
    /// Query kmers built successfully
    ///
    UCPLACE0005,
    //
    /// Query kmers map built successfully
    ///
    UCPLACE0006,
    //
    /// Root kmers map built successfully
    ///
    UCPLACE0007,
    //
    /// Expected clade coverage related message
    ///
    UCPLACE0008,
    //
    /// Starting tree introspection
    ///
    UCPLACE0009,
    //
    /// A introspection level was reached into the introspection loop
    ///
    UCPLACE0010,
    //
    /// A given clade has no enough coverage to be considered
    ///
    UCPLACE0011,
    //
    /// A one-vs-rest clade proposal set was built
    ///
    UCPLACE0012,
    //
    /// A one-vs-rest placement statistics was calculated
    ///
    UCPLACE0013,
    //
    /// A set of proposals are right to be be evaluated
    ///
    UCPLACE0014,
    //
    /// If empty clade proposal set was reached the `MaxResolutionReached` state
    /// is triggered
    ///
    UCPLACE0015,
    //
    /// If the clade proposal set contains just one clade, and a identity is
    /// reached
    ///
    UCPLACE0016,
    //
    /// If the clade proposal set contains just one clade, and the clade
    /// contains children clades, go to the next level
    ///
    UCPLACE0017,
    //
    /// Multiple clade proposal set was reached
    ///
    UCPLACE0018,
    //
    /// If multiple clade proposal set was reached, and the algorithm was unable
    /// to filter proposals to a conclusive one, the `Inconclusive` state is
    /// triggered
    ///
    UCPLACE0019,
    // ? -----------------------------------------------------------------------
}

impl Display for TelemetryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

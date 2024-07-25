use serde::Serialize;
use std::fmt::Display;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged, rename_all = "UPPERCASE")]
pub(crate) enum TelemetryCode {
    /// The placement is running
    ///
    /// The placement is running and the results are not available yet.
    WTHLACE0001,

    /// The placement is finished
    ///
    /// The placement is finished and the results are available.
    WTHPLACE0002,

    /// Classeq config loading
    ///
    /// Messages related to the loading of the Classeq configuration.
    WTHPLACE0003,

    /// Database config loading
    ///
    /// Messages related to the loading of the database configuration.
    WTHPLACE0004,

    /// Database loading
    ///
    /// Messages related to the loading of the database.
    WTHPLACE0005,

    /// Query file loading
    ///
    /// Messages related to the loading of the query file.
    WTHPLACE0006,

    /// Run file saving
    ///
    /// Messages related to the saving of the run file.
    WTHPLACE0007,

    /// Placement run
    ///
    /// Messages related to the placement run.
    WTHPLACE0008,

    /// Placement finished
    ///
    /// Messages related to the placement finished.
    WTHPLACE0009,
}

impl Display for TelemetryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

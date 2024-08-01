use serde::Serialize;
use std::fmt::Display;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged, rename_all = "UPPERCASE")]
pub(crate) enum TelemetryCode {
    CLIPLACE0001,
    CLIPLACE0002,
}

impl Display for TelemetryCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

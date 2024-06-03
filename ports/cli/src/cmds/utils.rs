use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OutputFormat {
    /// JSON format
    Json,

    /// YAML format
    Yaml,
}

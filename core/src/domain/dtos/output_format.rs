use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    /// JSONL format
    Jsonl,

    /// YAML format
    Yaml,
}

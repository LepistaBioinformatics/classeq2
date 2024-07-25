use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    /// Zstandard format
    ///
    /// The file will be compressed using Zstandard. See
    /// https://github.com/facebook/zstd for more information.
    Zstd,

    /// YAML format
    ///
    /// No compression will be applied to the file. The file will be saved in
    /// YAML format.
    Yaml,
}

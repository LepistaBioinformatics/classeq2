use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DatabaseOutputFormat {
    /// Zstandard format
    ///
    /// The file will be compressed using Zstandard. See
    /// `<https://github.com/facebook/zstd>` for more information.
    Zstd,

    /// YAML format
    ///
    /// No compression will be applied to the file. The file will be saved in
    /// YAML format.
    Yaml,

    /// JSON format
    ///
    /// No compression will be applied to the file. The file will be saved in
    /// JSON format.
    Json,

    /// Binary format
    ///
    /// The v2 binary database written by `build-db`. It is read without
    /// parsing, so it loads much faster than the other formats. Versions of
    /// cls that predate it cannot read it.
    Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DatabaseFormat {
    /// The v2 binary database
    Binary,

    /// The zstd-compressed YAML database read by earlier versions of cls
    Legacy,
}

#[derive(Clone, Debug, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DatabaseDescriptionOutputFormat {
    /// JSON format
    ///
    /// The file will be saved in JSON format.
    Json,

    /// YAML format
    ///
    /// The file will be saved in YAML format.
    Yaml,

    /// TSV format
    ///
    /// The file will be saved in TSV format.
    Tsv,
}

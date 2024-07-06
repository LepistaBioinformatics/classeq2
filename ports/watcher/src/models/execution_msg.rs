use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecutionMsg {
    pub(crate) msg: String,
}

impl ExecutionMsg {
    pub(crate) fn write_file(path: &PathBuf, msg: &str) -> std::io::Result<()> {
        match serde_yaml::to_string(&ExecutionMsg {
            msg: msg.to_owned(),
        }) {
            Ok(content) => std::fs::write(path, content),
            Err(e) => {
                eprintln!("Failed to serialize the error message: {}", e);
                Err(std::io::Error::new(std::io::ErrorKind::Other, e))
            }
        }
    }
}

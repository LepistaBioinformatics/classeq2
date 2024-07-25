use serde::Serialize;
use std::path::PathBuf;

use crate::dtos::telemetry_code::TelemetryCode;

#[derive(Clone, Debug, Serialize)]
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

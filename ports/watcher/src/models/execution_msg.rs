use anyhow::Result;
use serde::Serialize;
use std::{fs::File, io::Write, path::PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecutionMsg {
    pub(crate) msg: String,
}

impl ExecutionMsg {
    pub(crate) fn write_file(
        path: &PathBuf,
        msg: &str,
        is_logging: Option<bool>,
    ) -> Result<()> {
        match is_logging.unwrap_or(false) {
            true => {
                let mut output = match File::create(path) {
                    Ok(file) => file,
                    Err(e) => {
                        eprintln!(
                            "Failed to create the error message file: {e}"
                        );
                        return Err(e.into());
                    }
                };

                match write!(output, "{}", msg) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        eprintln!(
                            "Failed to write the error message to the file: {e}"
                        );
                        Err(e.into())
                    }
                }
            }
            false => match serde_yaml::to_string(&ExecutionMsg {
                msg: msg.to_owned(),
            }) {
                Ok(content) => match std::fs::write(path, content) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        eprintln!(
                            "Failed to write the error message to the file: {e}"
                        );
                        Err(e.into())
                    }
                },
                Err(e) => {
                    eprintln!("Failed to serialize the error message: {e}");
                    Err(e.into())
                }
            },
        }
    }
}

use anyhow::{Error, Result};
use classeq_core::domain::dtos::tree::Tree;
use std::{
    fs::{read_to_string, File},
    path::PathBuf,
};
use zstd::Decoder;

pub fn load_database(path: PathBuf) -> Result<Tree> {
    //
    // Read from yaml file
    //
    let read_from_yaml = |path: PathBuf| -> Result<Tree> {
        let content = read_to_string(path)?;
        match serde_yaml::from_str::<Tree>(&content.as_str()) {
            Err(err) => Err(Error::from(err)),
            Ok(buffer) => Ok(buffer),
        }
    };

    //
    // Read from binary file
    //
    let read_from_zstd = |path: PathBuf| -> Result<Tree> {
        let reader = File::open(path)?;
        let reader = Decoder::new(reader)?;
        match serde_yaml::from_reader(reader) {
            Err(err) => Err(Error::from(err)),
            Ok(buffer) => Ok(buffer),
        }
    };

    //
    // Load the database content
    //
    let tree_caller = |path: PathBuf| -> Result<Tree> {
        let bin_err = match read_from_zstd(path.to_owned()) {
            Ok(tree) => return Ok(tree),
            Err(err) => err,
        };

        let yaml_err = match read_from_yaml(path) {
            Ok(tree) => return Ok(tree),
            Err(err) => err,
        };

        Err(Error::msg(format!(
            "Error loading database: {bin_err} | {yaml_err}"
        )))
    };

    tree_caller(path)
}

use anyhow::{Error, Result};
use classeq_core::domain::dtos::tree::Tree;
use std::{
    fs::{read_to_string, File},
    io::Read,
    path::PathBuf,
};
use zstd::Decoder;

pub fn load_database(path: PathBuf) -> Result<Tree> {
    //
    // Read from yaml file
    //
    let read_from_yaml = |path: PathBuf| -> Result<Tree> {
        let content = read_to_string(path)?;
        match serde_saphyr::from_str_with_options::<Tree>(
            content.as_str(),
            Tree::yaml_options(),
        ) {
            Err(err) => Err(Error::from(err)),
            Ok(buffer) => Ok(buffer),
        }
    };

    //
    // Read from binary file
    //
    let read_from_zstd = |path: PathBuf| -> Result<Tree> {
        // Decompressing to a string and parsing it is faster than parsing
        // from the zstd stream (002-yaml-replacement R5).
        let mut content = String::new();
        Decoder::new(File::open(path)?)?.read_to_string(&mut content)?;
        match serde_saphyr::from_str_with_options::<Tree>(
            content.as_str(),
            Tree::yaml_options(),
        ) {
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

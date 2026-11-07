use anyhow::{Error, Result};
use classeq_core::domain::dtos::{
    kmer_index::{KmerIndex, MAGIC},
    tree::Tree,
};
use std::{
    fs::{read_to_string, File},
    io::Read,
    path::PathBuf,
    sync::Arc,
};
use zstd::Decoder;

/// Load a database in any format: the v2 binary file is mapped; a legacy
/// zstd YAML or YAML file is parsed and its k-mers frozen into the index
/// placement reads. The returned tree always has `index` set when the
/// database has k-mers, and never keeps `kmers_map`.
pub fn load_database(path: PathBuf) -> Result<Tree> {
    if starts_with_magic(&path) {
        let (mut tree, index) = KmerIndex::open(&path)?;
        tree.index = Some(Arc::new(index));
        return Ok(tree);
    }

    let mut tree = load_legacy(path)?;
    tree.freeze_kmers_map();
    Ok(tree)
}

fn starts_with_magic(path: &PathBuf) -> bool {
    let mut magic = [0u8; 8];

    File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok_and(|_| magic == MAGIC)
}

fn load_legacy(path: PathBuf) -> Result<Tree> {
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

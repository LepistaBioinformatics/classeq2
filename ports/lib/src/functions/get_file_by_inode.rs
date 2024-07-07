use std::{os::unix::fs::MetadataExt, path::PathBuf};
use walkdir::WalkDir;

pub fn get_file_by_inode(directory: PathBuf, inode: u32) -> Option<PathBuf> {
    WalkDir::new(directory)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            let metadata = entry.metadata().expect("Failed to read metadata");
            metadata.ino() == inode as u64
        })
        .map(|entry| entry.path().to_path_buf())
}

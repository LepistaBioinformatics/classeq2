use classeq_core::domain::dtos::clade::{Clade, NodeType};
use classeq_ports_lib::{get_file_by_inode, load_database};
use std::{os::unix::fs::MetadataExt, path::PathBuf};
use tempfile::TempDir;

const FIXTURE_ID: &str = "ce47d8bc-2885-3d2c-8247-5b8c8b28fefe";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/models")
        .join(name)
}

fn fixture_yaml() -> Vec<u8> {
    let compressed = std::fs::read(fixture("bsub-gyrb-k35.cls")).unwrap();
    zstd::decode_all(compressed.as_slice()).unwrap()
}

/// Serializes to JSON with every array sorted, so that values built from
/// `HashMap`/`HashSet` compare equal regardless of iteration order.
fn canonical<T: serde::Serialize>(value: &T) -> serde_json::Value {
    fn sort(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Array(items) => {
                let mut items: Vec<_> = items.into_iter().map(sort).collect();
                items.sort_by_key(|item| item.to_string());
                serde_json::Value::Array(items)
            }
            serde_json::Value::Object(map) => serde_json::Value::Object(
                map.into_iter().map(|(k, v)| (k, sort(v))).collect(),
            ),
            other => other,
        }
    }
    sort(serde_json::to_value(value).unwrap())
}

// ? ---------------------------------------------------------------------------
// ? LIB-01: load_database
// ? ---------------------------------------------------------------------------

#[test]
fn load_database_reads_shipped_zstd_fixture() {
    let tree = load_database(fixture("bsub-gyrb-k35.cls")).unwrap();

    assert_eq!(tree.id.to_string(), FIXTURE_ID);
    assert_eq!(tree.root.kind, NodeType::Root);
    assert_eq!(tree.min_branch_support, 70.0);

    let kmers_map = tree.kmers_map.expect("fixture must carry a kmers map");
    assert_eq!(kmers_map.get_kmer_size(), 35);
    assert_eq!(kmers_map.get_minimizer_size(), 4);
    assert!(!kmers_map.get_map().is_empty());
}

/// BUG-013: the fixture predates the removal of `inMemorySize`.
/// 002-yaml-replacement R2: a database written by serde_yaml (v0.10/v0.11)
/// loads with the new parser, and writing it back and reloading it yields
/// the same tree, k-mer map included.
#[test]
fn shipped_zstd_fixture_round_trips_through_the_current_serializer() {
    let original = load_database(fixture("bsub-gyrb-k35.cls")).unwrap();

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("rewritten.cls");
    let mut writer =
        zstd::Encoder::new(std::fs::File::create(&path).unwrap(), 0)
            .unwrap()
            .auto_finish();
    serde_saphyr::to_io_writer(&mut writer, &original).unwrap();
    drop(writer);

    let reloaded = load_database(path).unwrap();

    assert!(original.kmers_map.is_some());
    assert_eq!(canonical(&reloaded), canonical(&original));
}

#[test]
fn load_database_ignores_legacy_in_memory_size_key() {
    let yaml = String::from_utf8(fixture_yaml()).unwrap();
    assert!(yaml.contains("\ninMemorySize: "));

    let tree = load_database(fixture("bsub-gyrb-k35.cls")).unwrap();

    assert_eq!(tree.id.to_string(), FIXTURE_ID);
}

#[test]
fn load_database_reads_zstd_written_to_disk() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("db.cls");
    let compressed = zstd::encode_all(fixture_yaml().as_slice(), 0).unwrap();
    std::fs::write(&path, compressed).unwrap();

    let tree = load_database(path).unwrap();

    assert_eq!(tree.id.to_string(), FIXTURE_ID);
    assert!(tree.kmers_map.is_some());
}

#[test]
fn load_database_falls_back_to_plain_yaml() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("db.cls.yaml");
    std::fs::write(&path, fixture_yaml()).unwrap();

    let tree = load_database(path).unwrap();

    assert_eq!(tree.id.to_string(), FIXTURE_ID);
    assert_eq!(
        tree.kmers_map.unwrap().get_map().len(),
        load_database(fixture("bsub-gyrb-k35.cls"))
            .unwrap()
            .kmers_map
            .unwrap()
            .get_map()
            .len()
    );
}

#[test]
fn load_database_rejects_tree_only_yaml_fixture() {
    let err = load_database(fixture("bsub-gyrb-k35.cls.yaml"))
        .unwrap_err()
        .to_string();

    assert!(err.starts_with("Error loading database: "), "{err}");
    assert!(err.contains(" | "), "{err}");
}

#[test]
fn tree_only_yaml_fixture_is_a_root_clade() {
    let content =
        std::fs::read_to_string(fixture("bsub-gyrb-k35.cls.yaml")).unwrap();
    let root: Clade = serde_saphyr::from_str(&content).unwrap();

    assert_eq!(root.id, 0);
    assert_eq!(root.kind, NodeType::Root);
    assert!(root.children.is_some());
}

#[test]
fn load_database_reports_both_errors_for_garbage() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("garbage.cls");
    std::fs::write(&path, b"\x00\x01\x02 definitely: [not a database").unwrap();

    let err = load_database(path).unwrap_err().to_string();

    assert!(err.starts_with("Error loading database: "), "{err}");
    let (bin_err, yaml_err) = err
        .trim_start_matches("Error loading database: ")
        .split_once(" | ")
        .expect("both error halves must be present");
    assert!(!bin_err.is_empty());
    assert!(!yaml_err.is_empty());
}

#[test]
fn load_database_rejects_zstd_with_invalid_yaml() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.cls");
    let compressed = zstd::encode_all(&b"name: only-a-name\n"[..], 0).unwrap();
    std::fs::write(&path, compressed).unwrap();

    let err = load_database(path).unwrap_err().to_string();

    assert!(err.starts_with("Error loading database: "), "{err}");
}

#[test]
fn load_database_reports_missing_file() {
    let dir = TempDir::new().unwrap();

    let err = load_database(dir.path().join("missing.cls"))
        .unwrap_err()
        .to_string();

    assert_eq!(err.matches("No such file or directory").count(), 2, "{err}");
}

// ? ---------------------------------------------------------------------------
// ? LIB-02: get_file_by_inode
// ? ---------------------------------------------------------------------------

fn inode_of(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).unwrap().ino()
}

#[test]
fn get_file_by_inode_finds_nested_file() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    let target = nested.join("query.fasta");
    std::fs::write(&target, ">q\nACGT\n").unwrap();
    std::fs::write(dir.path().join("other.fasta"), ">o\nACGT\n").unwrap();

    let found = get_file_by_inode(dir.path().to_path_buf(), inode_of(&target));

    assert_eq!(found, Some(target));
}

#[test]
fn get_file_by_inode_returns_none_when_absent() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("present.fasta"), ">p\nACGT\n").unwrap();

    let elsewhere = TempDir::new().unwrap();
    let foreign = elsewhere.path().join("foreign.fasta");
    std::fs::write(&foreign, ">f\nACGT\n").unwrap();

    assert_eq!(
        get_file_by_inode(dir.path().to_path_buf(), inode_of(&foreign)),
        None
    );
}

#[test]
fn get_file_by_inode_returns_none_for_missing_directory() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("x");
    std::fs::write(&file, "x").unwrap();

    assert_eq!(
        get_file_by_inode(dir.path().join("missing"), inode_of(&file)),
        None
    );
}

#[test]
fn get_file_by_inode_does_not_truncate_inodes_to_32_bits() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("query.fasta");
    std::fs::write(&file, ">q\nACGT\n").unwrap();
    let inode = inode_of(&file);

    assert_eq!(get_file_by_inode(dir.path().to_path_buf(), inode), Some(file));
    assert_eq!(
        get_file_by_inode(dir.path().to_path_buf(), inode + (1 << 32)),
        None
    );
}

use super::{do_placement, scan_directories_in_background};
use crate::dtos::placement_result::PlacementResult;

use apalis::prelude::Data;
use classeq_ports_lib::{FileSystemConfig, ModelsConfig, PlacementConfig};
use std::{
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const MODEL_ID: &str = "ce47d8bc-2885-3d2c-8247-5b8c8b28fefe";
const FIXTURE_WD: &str = "019051d9-4c7a-7b2d-9dd1-66ef92236fd7";
const QUERY_NAME: &str =
    "bsub-refseq-sample50percentRemaining-clean-diamond.fasta";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_wd(id: &str) -> PathBuf {
    repo_root().join("tests/data/public").join(id)
}

fn model_path() -> PathBuf {
    repo_root().join("tests/models/bsub-gyrb-k35.cls")
}

fn fs_config(tmp: &TempDir) -> FileSystemConfig {
    FileSystemConfig {
        public_directory: "public".to_string(),
        serve_directory: tmp.path().join("srv").to_str().unwrap().to_string(),
        input_directory: "input".to_string(),
        output_directory: "output".to_string(),
        config_file_name: "config.yaml".to_string(),
        results_file_name: "result".to_string(),
        success_file_name: "success.yaml".to_string(),
        running_file_name: "running.yaml".to_string(),
        error_file_name: "error.yaml".to_string(),
        logging_file_name: "logging.jsonl".to_string(),
    }
}

fn models_config(model_path: &Path) -> ModelsConfig {
    serde_saphyr::from_str(&format!(
        "- id: {MODEL_ID}\n  name: bsub-gyrb\n  modelPath: {model}\n  annotationsPath: {ann}\n",
        model = model_path.display(),
        ann = repo_root()
            .join("tests/models/bsub-gyrb-annotations.yaml")
            .display(),
    ))
    .unwrap()
}

fn public_dir(fs: &FileSystemConfig) -> PathBuf {
    PathBuf::from(&fs.serve_directory).join(&fs.public_directory)
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn set_config_field(config: &Path, key: &str, value: serde_json::Value) {
    let mut yaml: serde_json::Value =
        serde_saphyr::from_str(&std::fs::read_to_string(config).unwrap())
            .unwrap();
    yaml[key] = value;
    std::fs::write(config, serde_saphyr::to_string(&yaml).unwrap()).unwrap();
}

/// Copies the pending part (config and input) of the fixture work dir into the
/// temporary public directory and points `queryFileId` to the copied file.
fn pending_work_dir(fs: &FileSystemConfig, id: &str) -> PathBuf {
    let wd = public_dir(fs).join(id);
    copy_dir(&fixture_wd(FIXTURE_WD).join("input"), &wd.join("input"));
    let config = wd.join("config.yaml");
    std::fs::copy(fixture_wd(FIXTURE_WD).join("config.yaml"), &config)
        .unwrap();

    let inode = std::fs::metadata(wd.join("input").join(QUERY_NAME))
        .unwrap()
        .ino();
    set_config_field(&config, "queryFileId", inode.into());

    wd
}

fn read_msg(path: &Path) -> String {
    let yaml: serde_json::Value =
        serde_saphyr::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    yaml["msg"].as_str().unwrap().to_string()
}

fn expect_error(
    result: PlacementResult<(String, PathBuf), (String, Option<PathBuf>)>,
) -> (String, Option<PathBuf>) {
    match result {
        PlacementResult::Error(err) => err,
        PlacementResult::Success((msg, _)) => {
            panic!("expected an error, got success: {msg}")
        }
    }
}

fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<(PathBuf, Vec<u8>)> = walkdir(dir)
        .into_iter()
        .map(|p| {
            let content = std::fs::read(&p).unwrap();
            (p, content)
        })
        .collect();
    files.sort();
    files
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walkdir(&path));
        } else {
            out.push(path);
        }
    }
    out
}

// ? ---------------------------------------------------------------------------
// ? WATCH-01: do_placement
// ? ---------------------------------------------------------------------------

#[test]
fn do_placement_places_the_fixture_query_and_writes_running_file() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let wd = pending_work_dir(&fs, FIXTURE_WD);

    let result = do_placement(
        wd.join("config.yaml"),
        &fs,
        &models_config(&model_path()),
        &tracing::Span::none(),
    );

    match result {
        PlacementResult::Success((msg, parent)) => {
            assert_eq!(parent, wd);
            assert_eq!(
                msg,
                format!("Query file \"{QUERY_NAME}\" processed successfully")
            );
        }
        PlacementResult::Error((msg, _)) => panic!("placement failed: {msg}"),
    }

    assert_eq!(
        read_msg(&wd.join("running.yaml")),
        format!(
            "Processing the query file \"{QUERY_NAME}\" with model {MODEL_ID}"
        )
    );

    let results = std::fs::read_to_string(wd.join("output/result.yaml"))
        .unwrap();
    assert!(!results.trim().is_empty());
}

#[test]
fn do_placement_reports_invalid_config_file() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let wd = pending_work_dir(&fs, FIXTURE_WD);
    std::fs::write(wd.join("config.yaml"), "name: [unterminated\n").unwrap();

    let (msg, parent) = expect_error(do_placement(
        wd.join("config.yaml"),
        &fs,
        &models_config(&model_path()),
        &tracing::Span::none(),
    ));

    assert!(
        msg.starts_with("Failed to parse the configuration file"),
        "{msg}"
    );
    assert_eq!(parent, None);
    assert!(!wd.join("running.yaml").exists());
}

#[test]
fn do_placement_reports_unknown_model() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let wd = pending_work_dir(&fs, FIXTURE_WD);
    let other = "00000000-0000-0000-0000-000000000001";
    set_config_field(&wd.join("config.yaml"), "modelId", other.into());

    let (msg, parent) = expect_error(do_placement(
        wd.join("config.yaml"),
        &fs,
        &models_config(&model_path()),
        &tracing::Span::none(),
    ));

    assert_eq!(msg, format!("Model with ID {other} not found"));
    assert_eq!(parent, None);
}

#[test]
fn do_placement_reports_unloadable_model() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let wd = pending_work_dir(&fs, FIXTURE_WD);
    let broken = tmp.path().join("broken.cls");
    std::fs::write(&broken, "not a database").unwrap();

    let (msg, parent) = expect_error(do_placement(
        wd.join("config.yaml"),
        &fs,
        &models_config(&broken),
        &tracing::Span::none(),
    ));

    assert!(
        msg.starts_with(&format!("Failed to load the model with ID {MODEL_ID}")),
        "{msg}"
    );
    assert_eq!(parent, None);
}

#[test]
fn do_placement_reports_missing_query_file_with_a_64_bit_inode() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let wd = pending_work_dir(&fs, FIXTURE_WD);
    let inode: u64 = 1 << 33;
    set_config_field(&wd.join("config.yaml"), "queryFileId", inode.into());

    let (msg, parent) = expect_error(do_placement(
        wd.join("config.yaml"),
        &fs,
        &models_config(&model_path()),
        &tracing::Span::none(),
    ));

    assert_eq!(msg, format!("Query file with inode {inode} not found"));
    assert_eq!(parent, None);
    assert!(!wd.join("running.yaml").exists());
}

#[test]
fn fixture_configs_deserialize_as_placement_config() {
    for id in [FIXTURE_WD, "019051d9-4c7a-7b2d-9dd1-66ef92236fd8"] {
        let config =
            PlacementConfig::from_yaml_file(&fixture_wd(id).join("config.yaml"))
                .unwrap();

        assert_eq!(config.model_id.to_string(), MODEL_ID);
        assert_eq!(config.work_dir, "019051d9-4c7a-7b2d-9dd1-66ef92236fd8");
    }
}

// ? ---------------------------------------------------------------------------
// ? WATCH-01: directory scanning
// ? ---------------------------------------------------------------------------

#[async_std::test]
async fn scan_skips_work_dirs_with_a_status_file() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let public = public_dir(&fs);

    copy_dir(&fixture_wd(FIXTURE_WD), &public.join(FIXTURE_WD));
    for (id, status) in [
        ("019051d9-0000-7000-8000-000000000001", "success.yaml"),
        ("019051d9-0000-7000-8000-000000000002", "running.yaml"),
        ("019051d9-0000-7000-8000-000000000003", "error.yaml"),
    ] {
        let wd = pending_work_dir(&fs, id);
        std::fs::write(wd.join(status), "msg: done\n").unwrap();
    }
    std::fs::create_dir_all(public.join("no-config")).unwrap();

    let before = snapshot(&public);

    scan_directories_in_background(
        Data::new(fs.to_owned()),
        Data::new(models_config(&model_path())),
    )
    .await;

    assert_eq!(snapshot(&public), before);
}

#[async_std::test]
async fn scan_processes_pending_work_dir_and_writes_success_and_logging() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let wd = pending_work_dir(&fs, FIXTURE_WD);

    scan_directories_in_background(
        Data::new(fs.to_owned()),
        Data::new(models_config(&model_path())),
    )
    .await;

    assert_eq!(
        read_msg(&wd.join("success.yaml")),
        format!("Query file \"{QUERY_NAME}\" processed successfully")
    );
    assert!(wd.join("running.yaml").exists());
    assert!(!wd.join("error.yaml").exists());
    assert!(wd.join("output/result.yaml").exists());

    let logging = std::fs::read_to_string(wd.join("logging.jsonl")).unwrap();
    assert!(logging.contains("WTHPLACE0007"), "{logging}");
    for line in logging.lines() {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
}

/// BUG-043: the captured log buffer was read before the non-blocking appender
/// flushed it, so the logging file was intermittently empty.
#[async_std::test]
async fn scan_writes_error_and_logging_files_on_failure() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);
    let other = "00000000-0000-0000-0000-000000000001";
    let work_dirs: Vec<PathBuf> = (0..8)
        .map(|i| {
            let wd = pending_work_dir(
                &fs,
                &format!("019051d9-0000-7000-8000-00000000001{i}"),
            );
            set_config_field(&wd.join("config.yaml"), "modelId", other.into());
            wd
        })
        .collect();

    scan_directories_in_background(
        Data::new(fs.to_owned()),
        Data::new(models_config(&model_path())),
    )
    .await;

    for wd in work_dirs {
        assert_eq!(
            read_msg(&wd.join("error.yaml")),
            format!("Model with ID {other} not found")
        );
        assert!(!wd.join("success.yaml").exists());
        assert!(!wd.join("running.yaml").exists());

        let logging =
            std::fs::read_to_string(wd.join("logging.jsonl")).unwrap();
        assert!(logging.contains("WTHPLACE0004"), "{wd:?}: {logging:?}");
    }
}

#[async_std::test]
async fn scan_tolerates_a_missing_public_directory() {
    let tmp = TempDir::new().unwrap();
    let fs = fs_config(&tmp);

    scan_directories_in_background(
        Data::new(fs.to_owned()),
        Data::new(models_config(&model_path())),
    )
    .await;

    assert!(!public_dir(&fs).exists());
}

use crate::{
    dtos::{
        log_writer::VectorWriter, placement_result::PlacementResult,
        telemetry_code::TelemetryCode,
    },
    models::{
        config_file::ConfigFile, execution_msg::ExecutionMsg,
        reminder::Reminder,
    },
};

use chrono::{TimeZone, Utc};
use std::{io::Write, path::PathBuf};
use tempfile::TempDir;

// ? ---------------------------------------------------------------------------
// ? WATCH-01: ExecutionMsg
// ? ---------------------------------------------------------------------------

#[test]
fn execution_msg_writes_yaml_by_default() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("success.yaml");

    ExecutionMsg::write_file(&path, "done: \"ok\"", None).unwrap();

    let yaml: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap();
    assert_eq!(yaml["msg"], "done: \"ok\"");
}

#[test]
fn execution_msg_writes_raw_content_when_logging() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("logging.jsonl");

    ExecutionMsg::write_file(&path, "{\"a\":1}\n", Some(true)).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":1}\n");
}

#[test]
fn execution_msg_overwrites_existing_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("logging.jsonl");
    std::fs::write(&path, "previous content that is longer").unwrap();

    ExecutionMsg::write_file(&path, "new", Some(true)).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[test]
fn execution_msg_fails_when_parent_dir_is_missing() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("missing/out.yaml");

    assert!(ExecutionMsg::write_file(&path, "x", None).is_err());
    assert!(ExecutionMsg::write_file(&path, "x", Some(true)).is_err());
}

// ? ---------------------------------------------------------------------------
// ? WATCH-01: ConfigFile
// ? ---------------------------------------------------------------------------

#[test]
fn config_file_parses_camel_case_yaml() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("watcher.yaml");
    std::fs::write(
        &path,
        "\
fs:
  publicDirectory: public
  serveDirectory: /srv
  inputDirectory: input
  outputDirectory: output
  configFileName: config.yaml
  resultsFileName: result
  successFileName: success.yaml
  runningFileName: running.yaml
  errorFileName: error.yaml
  loggingFileName: logging.jsonl
watcher:
  workerName: placement
  workers: 2
  interval: 5
  retries: 3
  maxThreads: 4
models:
  - id: ce47d8bc-2885-3d2c-8247-5b8c8b28fefe
    name: bsub-gyrb
    modelPath: /models/bsub.cls
",
    )
    .unwrap();

    let config = ConfigFile::from_file(&path).unwrap();

    assert_eq!(config.watcher.worker_name, "placement");
    assert_eq!(config.watcher.workers, 2);
    assert_eq!(config.watcher.interval, 5);
    assert_eq!(config.watcher.retries, 3);
    assert_eq!(config.watcher.max_threads, 4);
    assert_eq!(config.fs.success_file_name, "success.yaml");
    let models = config.models.get_models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_path(), PathBuf::from("/models/bsub.cls"));
    assert_eq!(models[0].annotations_path(), None);
}

#[test]
fn config_file_fails_on_missing_or_invalid_file() {
    let tmp = TempDir::new().unwrap();
    assert!(ConfigFile::from_file(&tmp.path().join("missing.yaml")).is_err());

    let path = tmp.path().join("invalid.yaml");
    std::fs::write(&path, "watcher:\n  workers: 1\n").unwrap();
    assert!(ConfigFile::from_file(&path).is_err());
}

// ? ---------------------------------------------------------------------------
// ? WATCH-01: dtos
// ? ---------------------------------------------------------------------------

#[test]
fn telemetry_code_displays_variant_name() {
    let codes = [
        TelemetryCode::WTHPLACE0001,
        TelemetryCode::WTHPLACE0002,
        TelemetryCode::WTHPLACE0003,
        TelemetryCode::WTHPLACE0004,
        TelemetryCode::WTHPLACE0005,
        TelemetryCode::WTHPLACE0006,
        TelemetryCode::WTHPLACE0007,
        TelemetryCode::WTHPLACE0008,
    ];

    for (i, code) in codes.iter().enumerate() {
        assert_eq!(code.to_string(), format!("WTHPLACE000{}", i + 1));
    }
}

#[test]
fn vector_writer_clones_share_one_buffer() {
    let writer = VectorWriter::new();
    let mut clone = writer.to_owned();

    clone.write_all(b"hello ").unwrap();
    clone.write_all(b"world").unwrap();
    clone.flush().unwrap();

    assert_eq!(*writer.get_buffer().lock().unwrap(), b"hello world");
}

#[test]
fn placement_result_carries_success_and_error_payloads() {
    let results: Vec<PlacementResult<u8, &str>> =
        vec![PlacementResult::Success(1), PlacementResult::Error("e")];

    let rendered: Vec<String> = results
        .into_iter()
        .map(|r| match r {
            PlacementResult::Success(v) => format!("ok:{v}"),
            PlacementResult::Error(e) => format!("err:{e}"),
        })
        .collect();

    assert_eq!(rendered, vec!["ok:1", "err:e"]);
}

#[test]
fn reminder_is_built_from_a_timestamp() {
    let t = Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap();

    let reminder = Reminder::from(t);

    assert!(format!("{reminder:?}").contains("2024-01-02T03:04:05Z"));
}

use classeq_core::domain::dtos::output_format::OutputFormat;
use classeq_ports_lib::{
    FileSystemConfig, LogFormat, ModelsConfig, PlacementConfig,
};
use std::path::PathBuf;
use tempfile::TempDir;

const FS_YAML: &str = r#"
serveDirectory: "tests/data"
publicDirectory: "public"
inputDirectory: "input"
outputDirectory: "output"
configFileName: "config.yaml"
resultsFileName: "result.yaml"
successFileName: "success.yaml"
runningFileName: "running.yaml"
loggingFileName: logging.jsonl
errorFileName: "error.yaml"
"#;

const MODELS_YAML: &str = r#"
- id: ce47d8bc-2885-3d2c-8247-5b8c8b28fefe
  name: Bacillus subtilis group
  gene: gyrB complete sequence
  modelPath: tests/models/bsub-gyrb-k35.cls
  annotationsPath: tests/models/bsub-gyrb-annotations.yaml
- id: 019051d9-4c7a-7b2d-9dd1-66ef92236fd7
  name: Minimal model
  modelPath: models/minimal.cls
"#;

const PLACEMENT_YAML: &str = r#"
name: test-analysis
email: sgelias@outlook.com
queryFileId: 29622357
modelId: ce47d8bc-2885-3d2c-8247-5b8c8b28fefe
outputFormat: yaml
workDir: 019051d9-4c7a-7b2d-9dd1-66ef92236fd8
"#;

#[test]
fn file_system_config_deserializes_from_settings_shape() {
    let config: FileSystemConfig = serde_yaml::from_str(FS_YAML).unwrap();

    assert_eq!(config.serve_directory, "tests/data");
    assert_eq!(config.public_directory, "public");
    assert_eq!(config.input_directory, "input");
    assert_eq!(config.output_directory, "output");
    assert_eq!(config.config_file_name, "config.yaml");
    assert_eq!(config.results_file_name, "result.yaml");
    assert_eq!(config.success_file_name, "success.yaml");
    assert_eq!(config.running_file_name, "running.yaml");
    assert_eq!(config.logging_file_name, "logging.jsonl");
    assert_eq!(config.error_file_name, "error.yaml");
}

#[test]
fn file_system_config_rejects_missing_field() {
    let partial = FS_YAML.replace("errorFileName: \"error.yaml\"", "");

    assert!(serde_yaml::from_str::<FileSystemConfig>(&partial).is_err());
}

#[test]
fn models_config_deserializes_with_optional_fields() {
    let config: ModelsConfig = serde_yaml::from_str(MODELS_YAML).unwrap();
    let models = config.get_models();

    assert_eq!(models.len(), 2);

    assert_eq!(
        models[0].id.to_string(),
        "ce47d8bc-2885-3d2c-8247-5b8c8b28fefe"
    );
    assert_eq!(models[0].name, "Bacillus subtilis group");
    assert_eq!(models[0].gene.as_deref(), Some("gyrB complete sequence"));
    assert_eq!(
        models[0].model_path(),
        PathBuf::from("tests/models/bsub-gyrb-k35.cls")
    );
    assert_eq!(
        models[0].annotations_path(),
        Some(PathBuf::from("tests/models/bsub-gyrb-annotations.yaml"))
    );

    assert_eq!(models[1].gene, None);
    assert_eq!(models[1].model_path(), PathBuf::from("models/minimal.cls"));
    assert_eq!(models[1].annotations_path(), None);
}

#[test]
fn models_config_serialization_hides_paths() {
    let config: ModelsConfig = serde_yaml::from_str(MODELS_YAML).unwrap();
    let value = serde_yaml::to_value(&config).unwrap();
    let first = &value[0];

    assert!(first.get("modelPath").is_none());
    assert!(first.get("annotationsPath").is_none());
    assert_eq!(first["name"].as_str(), Some("Bacillus subtilis group"));
    assert!(value[1].get("gene").is_none());
}

#[test]
fn models_config_rejects_invalid_uuid() {
    let invalid = MODELS_YAML
        .replace("ce47d8bc-2885-3d2c-8247-5b8c8b28fefe", "not-a-uuid");

    assert!(serde_yaml::from_str::<ModelsConfig>(&invalid).is_err());
}

#[test]
fn placement_config_deserializes_watcher_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/data/public/019051d9-4c7a-7b2d-9dd1-66ef92236fd7/config.yaml",
    );

    let config = PlacementConfig::from_yaml_file(&path).unwrap();

    assert_eq!(config.name, "test-analysis");
    assert_eq!(config.email, "sgelias@outlook.com");
    assert_eq!(config.query_file_id, 29622357);
    assert_eq!(
        config.model_id.to_string(),
        "ce47d8bc-2885-3d2c-8247-5b8c8b28fefe"
    );
    assert!(matches!(config.output_format, OutputFormat::Yaml));
    assert_eq!(config.work_dir, "019051d9-4c7a-7b2d-9dd1-66ef92236fd8");
    assert_eq!(config.description, None);
    assert_eq!(config.overwrite, None);
    assert_eq!(config.remove_intersection, None);
}

#[test]
fn placement_config_reads_optional_fields_and_jsonl_format() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.yaml");
    let content = PLACEMENT_YAML
        .replace("outputFormat: yaml", "outputFormat: jsonl")
        + "description: a run\noverwrite: true\nremoveIntersection: false\n";
    std::fs::write(&path, content).unwrap();

    let config = PlacementConfig::from_yaml_file(&path).unwrap();

    assert!(matches!(config.output_format, OutputFormat::Jsonl));
    assert_eq!(config.description.as_deref(), Some("a run"));
    assert_eq!(config.overwrite, Some(true));
    assert_eq!(config.remove_intersection, Some(false));
}

#[test]
fn placement_config_serialization_skips_absent_options() {
    let config: PlacementConfig = serde_yaml::from_str(PLACEMENT_YAML).unwrap();
    let value = serde_yaml::to_value(&config).unwrap();

    assert!(value.get("description").is_none());
    assert!(value.get("overwrite").is_none());
    assert!(value.get("removeIntersection").is_none());
    assert_eq!(value["queryFileId"].as_u64(), Some(29622357));
}

#[test]
fn placement_config_errors_on_missing_file() {
    let dir = TempDir::new().unwrap();

    assert!(
        PlacementConfig::from_yaml_file(&dir.path().join("missing.yaml"))
            .is_err()
    );
}

#[test]
fn log_format_deserializes_camel_case_values() {
    assert!(matches!(
        serde_yaml::from_str::<LogFormat>("ansi").unwrap(),
        LogFormat::Ansi
    ));
    assert!(matches!(
        serde_yaml::from_str::<LogFormat>("jsonl").unwrap(),
        LogFormat::Jsonl
    ));
    assert!(serde_yaml::from_str::<LogFormat>("Ansi").is_err());
    assert!(serde_yaml::from_str::<LogFormat>("text").is_err());
}

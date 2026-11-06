use super::{health_check, routes};
use crate::models::{api_config::ApiConfig, node::Node};

use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceResponse},
    http::StatusCode,
    test as atest, web, App,
};
use classeq_ports_lib::{FileSystemConfig, ModelsConfig, PlacementConfig};
use std::{
    collections::HashMap,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tempfile::TempDir;
use uuid::Uuid;

const BOUNDARY: &str = "classeqtestboundary";
const MODEL_ID: &str = "ce47d8bc-2885-3d2c-8247-5b8c8b28fefe";

struct Env {
    tmp: TempDir,
    fs: FileSystemConfig,
}

impl Env {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let serve = tmp.path().join("srv");
        std::fs::create_dir_all(serve.join("public")).unwrap();

        let fs = FileSystemConfig {
            public_directory: "public".to_string(),
            serve_directory: serve.to_str().unwrap().to_string(),
            input_directory: "input".to_string(),
            output_directory: "output".to_string(),
            config_file_name: "config.yaml".to_string(),
            results_file_name: "result".to_string(),
            success_file_name: "success.yaml".to_string(),
            running_file_name: "running.yaml".to_string(),
            error_file_name: "error.yaml".to_string(),
            logging_file_name: "logging.jsonl".to_string(),
        };

        Self { tmp, fs }
    }

    fn serve(&self) -> PathBuf {
        PathBuf::from(&self.fs.serve_directory)
    }

    fn public(&self) -> PathBuf {
        self.serve().join("public")
    }

    fn work_dir(&self, id: &str) -> PathBuf {
        self.public().join(id)
    }

    fn new_work_dir(&self, with_input: bool) -> String {
        let id = Uuid::now_v7().to_string();
        let wd = self.work_dir(&id);
        std::fs::create_dir_all(&wd).unwrap();
        if with_input {
            std::fs::create_dir_all(wd.join("input")).unwrap();
        }
        id
    }

    async fn app(
        &self,
    ) -> impl Service<
        actix_http::Request,
        Response = ServiceResponse<impl MessageBody>,
        Error = actix_web::Error,
    > {
        let models: ModelsConfig = serde_saphyr::from_str(&format!(
            "- id: {MODEL_ID}\n  name: bsub-gyrb\n  gene: gyrB\n  modelPath: /nonexistent/model.cls\n"
        ))
        .unwrap();

        atest::init_service(
            App::new()
                .app_data(web::Data::new(Mutex::new(self.fs.clone())))
                .app_data(web::Data::new(Mutex::new(models)))
                .configure(routes)
                .default_service(web::get().to(health_check)),
        )
        .await
    }
}

fn multipart_body(field: &str, file_name: &str, content: &str) -> Vec<u8> {
    format!(
        "--{BOUNDARY}\r\n\
         Content-Disposition: form-data; name=\"{field}\"; filename=\"{file_name}\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n\
         {content}\r\n\
         --{BOUNDARY}--\r\n"
    )
    .into_bytes()
}

fn upload_request(uri: &str, file_name: &str, content: &str) -> actix_http::Request {
    atest::TestRequest::post()
        .uri(uri)
        .insert_header((
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        ))
        .set_payload(multipart_body("file", file_name, content))
        .to_request()
}

fn placement_config_json(work_dir: &str, query_file_id: u64) -> serde_json::Value {
    serde_json::json!({
        "name": "test-analysis",
        "email": "user@example.com",
        "queryFileId": query_file_id,
        "modelId": MODEL_ID,
        "outputFormat": "yaml",
        "workDir": work_dir,
    })
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect()
}

// ? ---------------------------------------------------------------------------
// ? API-01: models
// ? ---------------------------------------------------------------------------

#[test]
fn api_config_from_file_parses_camel_case_yaml() {
    let env = Env::new();
    let path = env.tmp.path().join("api.yaml");
    std::fs::write(
        &path,
        format!(
            "fs:\n{fs}server:\n  address: 0.0.0.0\n  port: 8080\n  workers: 2\nmodels:\n  - id: {MODEL_ID}\n    name: m\n    modelPath: /m.cls\n",
            fs = serde_saphyr::to_string(&env.fs)
                .unwrap()
                .lines()
                .map(|l| format!("  {l}\n"))
                .collect::<String>()
        ),
    )
    .unwrap();

    let config = ApiConfig::from_file(&path).unwrap();

    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.workers, Some(2));
    assert_eq!(config.fs.public_directory, "public");
    assert_eq!(config.models.get_models().len(), 1);
}

#[test]
fn api_config_from_file_fails_on_missing_file() {
    assert!(ApiConfig::from_file(&PathBuf::from("/nonexistent/api.yaml")).is_err());
}

#[test]
fn node_new_strips_the_work_dir_prefix_and_keeps_the_full_inode() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let file = env.work_dir(&id).join("input/query.fasta");
    std::fs::write(&file, ">q\nACGT\n").unwrap();

    let node = Node::new(file.to_owned(), id).unwrap();

    assert_eq!(node.name, "input/query.fasta");
    assert_eq!(node.size, 8);
    assert_eq!(node.id, std::fs::metadata(&file).unwrap().ino());
    assert!(node.updated_at.is_some());
}

// ? ---------------------------------------------------------------------------
// ? API-01: init_wd
// ? ---------------------------------------------------------------------------

#[actix_web::test]
async fn init_wd_creates_a_uuid_work_directory() {
    let env = Env::new();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::post().uri("/wd").to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: HashMap<String, String> = atest::read_body_json(resp).await;
    let id = &body["workDirId"];
    assert!(Uuid::parse_str(id).is_ok());
    assert!(env.work_dir(id).is_dir());
}

#[actix_web::test]
async fn unknown_route_falls_back_to_health_check() {
    let env = Env::new();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get().uri("/health").to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(atest::read_body(resp).await, "healthy");
}

#[actix_web::test]
async fn list_available_models_hides_model_paths() {
    let env = Env::new();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get().uri("/models").to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = atest::read_body_json(resp).await;
    assert_eq!(body[0]["id"], MODEL_ID);
    assert_eq!(body[0]["gene"], "gyrB");
    assert!(body[0].get("modelPath").is_none());
}

// ? ---------------------------------------------------------------------------
// ? API-01: list_wd_content
// ? ---------------------------------------------------------------------------

#[actix_web::test]
async fn list_wd_content_returns_404_for_unknown_work_dir() {
    let env = Env::new();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get()
            .uri(&format!("/wd/{}", Uuid::now_v7()))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn list_wd_content_returns_204_without_input_dir() {
    let env = Env::new();
    let id = env.new_work_dir(false);
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get().uri(&format!("/wd/{id}")).to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn list_wd_content_lists_files_of_the_work_dir() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let wd = env.work_dir(&id);
    std::fs::write(wd.join("input/query.fasta"), ">q\nACGT\n").unwrap();
    std::fs::write(wd.join("config.yaml"), "name: x\n").unwrap();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get().uri(&format!("/wd/{id}")).to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let nodes: Vec<Node> = atest::read_body_json(resp).await;
    let mut names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["config.yaml", "input/query.fasta"]);
}

// ? ---------------------------------------------------------------------------
// ? API-01: get_file_content_by_id
// ? ---------------------------------------------------------------------------

#[actix_web::test]
async fn get_file_content_by_id_serves_the_listed_file() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let file = env.work_dir(&id).join("input/query.fasta");
    std::fs::write(&file, ">q\nACGT\n").unwrap();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get().uri(&format!("/wd/{id}")).to_request(),
    )
    .await;
    let nodes: Vec<Node> = atest::read_body_json(resp).await;
    let node = nodes.iter().find(|n| n.name == "input/query.fasta").unwrap();

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get()
            .uri(&format!("/wd/{id}/{}", node.id))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(atest::read_body(resp).await, ">q\nACGT\n");
}

#[actix_web::test]
async fn get_file_content_by_id_returns_204_for_unknown_inode() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let elsewhere = env.tmp.path().join("foreign.txt");
    std::fs::write(&elsewhere, "secret").unwrap();
    let foreign_inode = std::fs::metadata(&elsewhere).unwrap().ino();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::get()
            .uri(&format!("/wd/{id}/{foreign_inode}"))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

/// BUG-042 (S-13): inode ids above `i32::MAX` were rejected by the path
/// extractor and inodes above `u32::MAX` were truncated.
#[actix_web::test]
async fn get_file_content_by_id_accepts_inodes_wider_than_32_bits() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let app = env.app().await;

    for file_id in [3_000_000_000_u64, u32::MAX as u64 + 1] {
        let resp = atest::call_service(
            &app,
            atest::TestRequest::get()
                .uri(&format!("/wd/{id}/{file_id}"))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::NO_CONTENT, "{file_id}");
    }
}

// ? ---------------------------------------------------------------------------
// ? API-01: upload_analysis_file
// ? ---------------------------------------------------------------------------

#[actix_web::test]
async fn upload_analysis_file_writes_into_input_dir() {
    let env = Env::new();
    let id = env.new_work_dir(false);
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        upload_request(&format!("/wd/{id}"), "query.fasta", ">q\nACGT"),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_eq!(
        std::fs::read_to_string(env.work_dir(&id).join("input/query.fasta"))
            .unwrap(),
        ">q\nACGT"
    );
}

#[actix_web::test]
async fn upload_analysis_file_conflicts_without_force_and_overwrites_with_it() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let target = env.work_dir(&id).join("input/query.fasta");
    std::fs::write(&target, "old").unwrap();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        upload_request(&format!("/wd/{id}"), "query.fasta", "new"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");

    let resp = atest::call_service(
        &app,
        upload_request(&format!("/wd/{id}?force=true"), "query.fasta", "new"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
}

#[actix_web::test]
async fn upload_analysis_file_returns_404_for_unknown_work_dir() {
    let env = Env::new();
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        upload_request(&format!("/wd/{}", Uuid::now_v7()), "q.fasta", "x"),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// BUG-040: the upload file name comes from the client and was joined to the
/// input directory without validation.
#[actix_web::test]
async fn upload_analysis_file_rejects_file_names_escaping_the_input_dir() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let absolute = env.tmp.path().join("absolute.txt");
    let app = env.app().await;

    for name in [
        "../../escaped.txt".to_string(),
        "../config.yaml".to_string(),
        "sub/../../x.txt".to_string(),
        absolute.to_str().unwrap().to_string(),
        "..".to_string(),
    ] {
        let resp = atest::call_service(
            &app,
            upload_request(&format!("/wd/{id}"), &name, "pwned"),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{name}");
    }

    let input = env.work_dir(&id).join("input");
    let outside: Vec<PathBuf> = files_under(env.tmp.path())
        .into_iter()
        .filter(|p| !p.starts_with(&input))
        .collect();
    assert!(outside.is_empty(), "{outside:?}");
}

// ? ---------------------------------------------------------------------------
// ? API-01: configure_placement_analysis
// ? ---------------------------------------------------------------------------

#[actix_web::test]
async fn configure_placement_analysis_writes_a_config_the_watcher_can_read() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::post()
            .uri(&format!("/wd/{id}/config"))
            .set_json(placement_config_json(&id, 42))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let config = PlacementConfig::from_yaml_file(
        &env.work_dir(&id).join("config.yaml"),
    )
    .unwrap();
    assert_eq!(config.query_file_id, 42);
    assert_eq!(config.model_id.to_string(), MODEL_ID);
    assert_eq!(config.work_dir, id);
}

#[actix_web::test]
async fn configure_placement_analysis_rejects_invalid_body() {
    let env = Env::new();
    let id = env.new_work_dir(true);
    let app = env.app().await;

    let resp = atest::call_service(
        &app,
        atest::TestRequest::post()
            .uri(&format!("/wd/{id}/config"))
            .set_json(serde_json::json!({"name": "x"}))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(!env.work_dir(&id).join("config.yaml").exists());
}

#[actix_web::test]
async fn configure_placement_analysis_returns_404_for_unknown_work_dir() {
    let env = Env::new();
    let app = env.app().await;
    let id = Uuid::now_v7().to_string();

    let resp = atest::call_service(
        &app,
        atest::TestRequest::post()
            .uri(&format!("/wd/{id}/config"))
            .set_json(placement_config_json(&id, 1))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ? ---------------------------------------------------------------------------
// ? API-01: work directory id traversal (BUG-041)
// ? ---------------------------------------------------------------------------

const TRAVERSAL_IDS: [&str; 3] = ["..", "%2E%2E", "."];

#[actix_web::test]
async fn list_wd_content_rejects_traversal_ids() {
    let env = Env::new();
    let other = env.new_work_dir(true);
    std::fs::write(env.work_dir(&other).join("input/q.fasta"), "x").unwrap();
    std::fs::create_dir_all(env.serve().join("input")).unwrap();
    std::fs::create_dir_all(env.public().join("input")).unwrap();
    let app = env.app().await;

    for wd in TRAVERSAL_IDS {
        let resp = atest::call_service(
            &app,
            atest::TestRequest::get().uri(&format!("/wd/{wd}")).to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{wd}");
    }
}

#[actix_web::test]
async fn get_file_content_by_id_rejects_traversal_ids() {
    let env = Env::new();
    let other = env.new_work_dir(true);
    let secret = env.work_dir(&other).join("input/secret.fasta");
    std::fs::write(&secret, "secret").unwrap();
    let inode = std::fs::metadata(&secret).unwrap().ino();
    let app = env.app().await;

    for wd in TRAVERSAL_IDS {
        let resp = atest::call_service(
            &app,
            atest::TestRequest::get()
                .uri(&format!("/wd/{wd}/{inode}"))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{wd}");
    }
}

#[actix_web::test]
async fn upload_analysis_file_rejects_traversal_ids() {
    let env = Env::new();
    let app = env.app().await;

    for wd in TRAVERSAL_IDS {
        let resp = atest::call_service(
            &app,
            upload_request(&format!("/wd/{wd}"), "q.fasta", "x"),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{wd}");
    }

    assert!(files_under(env.tmp.path()).is_empty());
}

#[actix_web::test]
async fn configure_placement_analysis_rejects_traversal_ids() {
    let env = Env::new();
    let app = env.app().await;

    for wd in TRAVERSAL_IDS {
        let resp = atest::call_service(
            &app,
            atest::TestRequest::post()
                .uri(&format!("/wd/{wd}/config"))
                .set_json(placement_config_json(wd, 1))
                .to_request(),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{wd}");
    }

    assert!(files_under(env.tmp.path()).is_empty());
}

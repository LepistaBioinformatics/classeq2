use crate::models::node::Node;

use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use classeq_ports_lib::{
    get_file_by_inode, BluAnalysisConfig, FileSystemConfig, ModelsConfig,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tokio::io::AsyncWriteExt;
use tracing::{error, instrument};
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Deserialize, Serialize, Debug)]
struct DirResponse {
    status: u32,
    msg: Option<String>,
}

fn check_directory_existence(
    fs_config: web::Data<Mutex<FileSystemConfig>>,
    work_dir_id: String,
    ignore_empty_input_dir: Option<bool>,
) -> Result<PathBuf, HttpResponse> {
    let ignore_empty_input_dir = ignore_empty_input_dir.unwrap_or(false);

    let data = match fs_config.lock() {
        Err(err) => {
            error!("{err}");

            return Err(HttpResponse::InternalServerError().json(
                DirResponse {
                    status: 404,
                    msg: Some(
                        "Unexpected error on try to process work directory"
                            .to_string(),
                    ),
                },
            ));
        }
        Ok(res) => res,
    };

    let base_dir = PathBuf::from(&data.serve_directory)
        .join(data.public_directory.clone())
        .join(work_dir_id.to_owned());

    if !base_dir.exists() {
        return Err(HttpResponse::NotFound().json(DirResponse {
            status: 404,
            msg: Some("Work directory not exists".to_string()),
        }));
    }

    let target_dir = base_dir.join(data.input_directory.clone());

    if !target_dir.exists() && !ignore_empty_input_dir {
        return Err(HttpResponse::NoContent().finish());
    }

    Ok(target_dir)
}

/// Initialize the work directory
///
#[instrument(name = "Initializing work directory", skip(config))]
pub(crate) async fn init_wd(
    config: web::Data<Mutex<FileSystemConfig>>,
) -> HttpResponse {
    let data = config.lock().unwrap();
    let path: PathBuf = PathBuf::from(&data.serve_directory);

    // TODO:
    //
    // Implement a way to build directory from the user's identity
    // extracted from the token.
    let target_prefix = data.public_directory.clone();
    let directory_id = Uuid::now_v7().to_string();
    let target_dir = path.join(target_prefix).join(directory_id.to_owned());

    if let Err(err) = std::fs::create_dir_all(&target_dir) {
        return HttpResponse::InternalServerError().body(err.to_string());
    };

    HttpResponse::Created()
        .json(HashMap::from([("workDirId".to_string(), directory_id)]))
}

#[instrument(name = "List work dir content", skip(config))]
pub(crate) async fn list_wd_content(
    work_dir_id: web::Path<String>,
    config: web::Data<Mutex<FileSystemConfig>>,
) -> HttpResponse {
    let work_dir_id = work_dir_id.into_inner();

    let target_dir =
        match check_directory_existence(config, work_dir_id.to_owned(), None) {
            Err(res) => return res,
            Ok(path) => path,
        };

    let directory_content: Vec<Node> =
        WalkDir::new(&target_dir.parent().unwrap_or(&target_dir))
            .contents_first(true)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().exists()
                    && (entry.path().is_file() || entry.path().is_symlink())
            })
            .filter_map(|entry| {
                match Node::new(entry.path().into(), work_dir_id.to_owned()) {
                    Ok(node) => Some(node),
                    Err(err) => {
                        error!("{:?}", err);
                        None
                    }
                }
            })
            .filter(|node| vec![""].contains(&node.name.as_str()) == false)
            .collect();

    HttpResponse::Ok().json(directory_content)
}

#[instrument(name = "Get file content", skip(config))]
pub(crate) async fn get_file_content_by_id(
    info: web::Path<(String, i32)>,
    config: web::Data<Mutex<FileSystemConfig>>,
    req: HttpRequest,
) -> HttpResponse {
    let (work_dir_id, file_id) = info.to_owned();

    let target_dir = match check_directory_existence(
        config,
        work_dir_id.to_owned(),
        Some(true),
    ) {
        Err(res) => return res,
        Ok(path) => path,
    };

    let parent = match target_dir.parent() {
        Some(parent) => parent,
        None => {
            return HttpResponse::InternalServerError().finish();
        }
    };

    match get_file_by_inode(parent.to_owned(), file_id as u32) {
        None => HttpResponse::NoContent().finish(),
        Some(file) => match NamedFile::open(file) {
            Ok(file) => file.into_response(&req),
            Err(err) => {
                error!("{:?}", err);
                HttpResponse::InternalServerError().finish()
            }
        },
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadAnalysisFileArgs {
    pub force: Option<bool>,
}

#[instrument(name = "Upload analysis file", skip(config, query, payload))]
pub(crate) async fn upload_analysis_file(
    work_dir_id: web::Path<String>,
    config: web::Data<Mutex<FileSystemConfig>>,
    query: web::Query<UploadAnalysisFileArgs>,
    request: HttpRequest,
    mut payload: Multipart,
) -> HttpResponse {
    let target_dir = match check_directory_existence(
        config,
        work_dir_id.into_inner(),
        Some(true),
    ) {
        Err(res) => return res,
        Ok(path) => path,
    };

    if let Err(err) = std::fs::create_dir_all(&target_dir) {
        error!("{:?}", err);
        return HttpResponse::InternalServerError().finish();
    };

    while let Some(field) = payload.next().await {
        let mut field = match field {
            Ok(field) => field,
            Err(err) => {
                error!("{:?}", err);
                return HttpResponse::BadRequest().body("Invalid request");
            }
        };

        let file_name = match field.content_disposition().get_filename() {
            Some(name) => name,
            None => return HttpResponse::BadRequest().body("Invalid request"),
        };

        let target_file = target_dir.join(file_name);

        if target_file.exists() {
            if !query.force.unwrap_or(false) {
                return HttpResponse::Conflict().json(DirResponse {
                    status: 409,
                    msg: Some(format!(
                        "\
    File already exists ({f}). If you want to overwrite it, use the `force` query \
    parameter.",
                        f = target_file
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap_or("unnamed")
                    )),
                });
            } else {
                if let Err(err) = std::fs::remove_file(&target_file) {
                    error!("{:?}", err);
                    return HttpResponse::InternalServerError().finish();
                };
            }
        }

        let mut file = match tokio::fs::File::create(target_file).await {
            Ok(file) => file,
            Err(err) => {
                error!("{:?}", err);
                return HttpResponse::InternalServerError().finish();
            }
        };

        if field.name() == "file" {
            while let Some(chunk) = field.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        error!("{:?}", err);
                        return HttpResponse::InternalServerError().finish();
                    }
                };

                if let Err(err) = file.write_all(&chunk).await {
                    error!("{:?}", err);
                    return HttpResponse::InternalServerError().finish();
                };
            }
        }
    }

    HttpResponse::Created().json(DirResponse {
        status: 201,
        msg: Some("File saved successfully".to_string()),
    })
}

#[instrument(name = "Configure Placement Analysis")]
pub(crate) async fn configure_placement_analysis(
    work_dir_id: web::Path<String>,
    fs_config: web::Data<Mutex<FileSystemConfig>>,
    trees_config: web::Data<Mutex<ModelsConfig>>,
    request: HttpRequest,
    body: web::Json<BluAnalysisConfig>,
) -> HttpResponse {
    let target_dir = match check_directory_existence(
        fs_config.to_owned(),
        work_dir_id.into_inner(),
        Some(true),
    ) {
        Err(res) => return res,
        Ok(path) => path,
    };

    let fs_config = match fs_config.lock() {
        Ok(res) => res,
        Err(err) => {
            error!("{:?}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let analysis_config = body.into_inner();

    let config_file_path = (if let Some(path) = target_dir.parent() {
        path
    } else {
        return HttpResponse::InternalServerError().finish();
    })
    .join(fs_config.to_owned().config_file_name);

    let config_file = match std::fs::File::create(config_file_path) {
        Ok(file) => file,
        Err(err) => {
            error!("{:?}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    if let Err(err) = serde_yaml::to_writer(config_file, &analysis_config) {
        error!("{:?}", err);
        return HttpResponse::InternalServerError().finish();
    };

    HttpResponse::Created().json(DirResponse {
        status: 201,
        msg: Some("Analysis configuration saved successfully".to_string()),
    })
}

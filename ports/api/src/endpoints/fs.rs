use crate::models::{
    analyses_config::BluAnalysisConfig,
    api_config::{AvailableTreesConfig, FileSystemConfig},
    node::Node,
};

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
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
        .join(data.anonymous_directory.clone())
        .join(work_dir_id.to_owned());

    if !base_dir.exists() {
        return Err(HttpResponse::NotFound().json(DirResponse {
            status: 404,
            msg: Some("Work directory not initialized".to_string()),
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
    let target_prefix = data.anonymous_directory.clone();
    let directory_id = Uuid::new_v4().to_string();
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
    let target_dir =
        match check_directory_existence(config, work_dir_id.into_inner(), None)
        {
            Err(res) => return res,
            Ok(path) => path,
        };

    let directory_content: Vec<Node> = WalkDir::new(&target_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().exists() &&
                (entry.path().is_dir() || entry.path().is_file())
        })
        .map(|entry| {
            Node::new(
                entry.path().into(),
                match target_dir.clone().as_os_str().to_str() {
                    None => entry.path().display().to_string(),
                    Some(path) => path.to_string(),
                },
            )
        })
        .filter(|node| vec![""].contains(&node.name.as_str()) == false)
        .collect();

    HttpResponse::Ok().json(directory_content)
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

        if target_file.exists() && !query.force.unwrap_or(false) {
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

    HttpResponse::Created().body("File saved successfully")
}

#[instrument(name = "Configure Blutils Analysis")]
pub(crate) async fn configure_blutils_analysis(
    work_dir_id: web::Path<String>,
    fs_config: web::Data<Mutex<FileSystemConfig>>,
    trees_config: web::Data<Mutex<AvailableTreesConfig>>,
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

    HttpResponse::Ok().finish()
}

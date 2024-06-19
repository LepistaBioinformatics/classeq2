use crate::models::{config::FileSystemConfig, node::Node};

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Result};
use futures::StreamExt;
use std::{path::PathBuf, sync::Mutex};
use tokio::io::AsyncWriteExt;
use tracing::{error, info, instrument};
use uuid::Uuid;
use walkdir::WalkDir;

#[instrument(name = "Initializing work directory", skip(config))]
pub(crate) async fn init_wd(
    config: web::Data<Mutex<FileSystemConfig>>,
) -> Result<String> {
    let data = config.lock().unwrap();
    let path: PathBuf = PathBuf::from(&data.serve_directory);

    // TODO:
    //
    // Implement a way to build directory from the user's identity
    // extracted from the token.
    let target_prefix = data.anonymous_directory.clone();

    let directory_id = Uuid::new_v4().to_string();
    let target_dir = path.join(target_prefix).join(directory_id.to_owned());
    std::fs::create_dir_all(&target_dir)?;

    info!("{:?}", target_dir);

    Ok(directory_id)
}

#[instrument(name = "List work dir content", skip(config))]
pub(crate) async fn list_wd_content(
    work_dir_id: web::Path<String>,
    config: web::Data<Mutex<FileSystemConfig>>,
) -> HttpResponse {
    let data = config.lock().unwrap();
    let path: PathBuf = PathBuf::from(&data.serve_directory);
    let work_dir_id = work_dir_id.into_inner();

    // TODO:
    //
    // Implement a way to build directory from the user's identity
    // extracted from the token.
    let target_dir = path
        .join(data.anonymous_directory.clone())
        .join(work_dir_id.to_owned())
        .join(data.input_directory.clone());

    let directory_content: Vec<Node> = WalkDir::new(&target_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
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

#[instrument(name = "Upload analysis file", skip(config, payload))]
pub(crate) async fn upload_analysis_file(
    work_dir_id: web::Path<String>,
    config: web::Data<Mutex<FileSystemConfig>>,
    request: HttpRequest,
    mut payload: Multipart,
) -> HttpResponse {
    let data = match config.lock() {
        Err(err) => {
            error!("{:?}", err);
            return HttpResponse::InternalServerError().finish();
        }
        Ok(data) => data,
    };

    let path = PathBuf::from(&data.to_owned().serve_directory);
    let work_dir_id = work_dir_id.into_inner();

    // TODO:
    //
    // Implement a way to build directory from the user's identity
    // extracted from the token.
    let target_dir = path
        .join(data.to_owned().anonymous_directory)
        .join(work_dir_id.to_owned())
        .join(data.to_owned().input_directory);

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
            None => {
                return HttpResponse::BadRequest().body("Invalid request");
            }
        };

        let mut file =
            match tokio::fs::File::create(target_dir.join(file_name)).await {
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

    HttpResponse::Ok().body("File saved successfully")
}

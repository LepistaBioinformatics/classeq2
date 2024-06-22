use crate::models::api_config::AvailableModelsConfig;

use actix_web::{web, HttpResponse};
use std::sync::Mutex;
use tracing::{error, instrument};

#[instrument(name = "List available trees", skip(config))]
pub(crate) async fn list_available_trees(
    config: web::Data<Mutex<AvailableModelsConfig>>,
) -> HttpResponse {
    match config.lock() {
        Err(err) => {
            error!("{:?}", err);
            return HttpResponse::InternalServerError().finish();
        }
        Ok(res) => {
            let trees = res.to_owned().models;
            HttpResponse::Ok().json(trees)
        }
    }
}

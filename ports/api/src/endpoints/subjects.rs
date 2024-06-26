use actix_web::{web, HttpResponse};
use classeq_ports_lib::ModelsConfig;
use std::sync::Mutex;
use tracing::{error, instrument};

#[instrument(name = "List available models", skip(config))]
pub(crate) async fn list_available_models(
    config: web::Data<Mutex<ModelsConfig>>,
) -> HttpResponse {
    match config.lock() {
        Err(err) => {
            error!("{:?}", err);
            return HttpResponse::InternalServerError().finish();
        }
        Ok(res) => {
            let trees = res.to_owned().get_models();
            HttpResponse::Ok().json(trees)
        }
    }
}

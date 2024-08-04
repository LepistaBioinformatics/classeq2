mod endpoints;
mod models;

use endpoints::fs;

use actix_web::{web, App, HttpResponse, HttpServer};
use actix_web_opentelemetry::RequestTracing;
use models::api_config::ApiConfig;
use std::{path::PathBuf, sync::Mutex};
use tracing::{info, subscriber::set_global_default};
use tracing_actix_web::TracingLogger;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().body("healthy")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // ? -----------------------------------------------------------------------
    // ? Initialize services configuration
    //
    // Configuration loaded here should be injected to children services.
    //
    // ? -----------------------------------------------------------------------

    info!("Initializing services configuration");

    let env_config_path = match std::env::var("SETTINGS_PATH") {
        Ok(path) => path,
        Err(err) => panic!("Error on get env `SETTINGS_PATH`: {err}"),
    };

    let config = match ApiConfig::from_file(&PathBuf::from(env_config_path)) {
        Ok(res) => res,
        Err(err) => panic!("Error on init config: {err}"),
    };

    let server_config = config.to_owned().server;
    let trees_config = config.to_owned().models;
    let fs_config = config.to_owned().fs;
    let workers = server_config.workers.unwrap_or(1);

    let address = (
        server_config.to_owned().address,
        server_config.to_owned().port,
    );

    // ? -----------------------------------------------------------------------
    // ? Initialize tracing
    // ? -----------------------------------------------------------------------

    std::env::set_var("RUST_LOG", "info,actix_web=error");

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let formatting_layer =
        BunyanFormattingLayer::new(PKG_NAME.into(), std::io::stdout);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);

    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");

    // ? -----------------------------------------------------------------------
    // ? Initialize api
    // ? -----------------------------------------------------------------------

    HttpServer::new(move || {
        App::new()
            .wrap(RequestTracing::new())
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(Mutex::new(fs_config.clone())))
            .app_data(web::Data::new(Mutex::new(trees_config.clone())))
            .route("/wd", web::post().to(fs::init_wd))
            .route("/wd/{work_dir_id}", web::get().to(fs::list_wd_content))
            .route(
                "/wd/{work_dir_id}",
                web::post().to(fs::upload_analysis_file),
            )
            .route(
                "/wd/{work_dir_id}/config",
                web::post().to(fs::configure_placement_analysis),
            )
            .route(
                "/wd/{work_dir_id}/{file_id}",
                web::get().to(fs::get_file_content_by_id),
            )
            .route(
                "/models",
                web::get().to(endpoints::subjects::list_available_models),
            )
            .default_service(web::get().to(health_check))
    })
    .bind(address)?
    .workers(workers.into())
    .run()
    .await
}

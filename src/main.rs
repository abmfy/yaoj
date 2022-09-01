use actix_web::{
    middleware::Logger,
    post,
    web::{Data, JsonConfig, PathConfig, QueryConfig},
    App, HttpServer, Responder,
};
use clap::Parser;
use env_logger;
use log;

mod api;
mod config;
mod judge;

use api::err::{Error, Reason};
use config::Args;

// DO NOT REMOVE: used in automatic testing
#[post("/internal/exit")]
#[allow(unreachable_code)]
async fn exit() -> impl Responder {
    log::info!("Shutdown as requested");
    std::process::exit(0);
    format!("Exited")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = Args::parse();
    let config = args.config.clone();

    // Config parameter extractor so that we return a unified JSON response when argument is invalid
    let query_cfg = QueryConfig::default()
        .error_handler(|err, _| Error::new(Reason::InvalidArgument, err.to_string()).into());
    let path_cfg = PathConfig::default()
        .error_handler(|err, _| Error::new(Reason::InvalidArgument, err.to_string()).into());
    let json_cfg = JsonConfig::default()
        .error_handler(|err, _| Error::new(Reason::InvalidArgument, err.to_string()).into());

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(Data::new(config.clone()))
            .app_data(query_cfg.clone())
            .app_data(path_cfg.clone())
            .app_data(json_cfg.clone())
            .service(api::jobs::new_job)
            .service(api::jobs::get_jobs)
            .service(api::jobs::get_job)
            .service(api::jobs::rejudge_job)
            // DO NOT REMOVE: used in automatic testing
            .service(exit)
    })
    .bind((
        args.config.server.bind_address.to_string(),
        args.config.server.bind_port,
    ))?
    .run()
    .await
}

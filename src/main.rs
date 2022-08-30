use actix_web::{middleware::Logger, post, web::Data, App, HttpServer, Responder};
use clap::Parser;
use env_logger;
use log;

mod api;
mod config;
mod judge;

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

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(Data::new(config.clone()))
            .service(api::jobs::new_job)
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

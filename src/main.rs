use actix_web::{
    middleware::Logger,
    post,
    web::{Data, JsonConfig, PathConfig, QueryConfig},
    App, HttpServer, Responder,
};
use clap::Parser;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    Connection, SqliteConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

mod api;
mod config;
mod judge;
mod persistent;

use api::err::{Error, Reason};
use config::Args;

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

const DB_URL: &str = "oj.db";
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

// DO NOT REMOVE: used in automatic testing
#[post("/internal/exit")]
#[allow(unreachable_code)]
async fn exit() -> impl Responder {
    log::info!("Shutdown as requested");
    std::process::exit(0);
    "Exited".to_string()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = Args::parse();
    let config = args.config.clone();

    // Delete existing database
    if args.flush_data {
        log::info!("Flushing persistent data");
        std::fs::remove_file(DB_URL).expect("Failed to remove database");
    }

    // Run migrations
    SqliteConnection::establish(DB_URL)
        .expect("Failed to establish database connection")
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Create connection pool
    let manager = ConnectionManager::<SqliteConnection>::new(DB_URL);
    let pool = Pool::new(manager).expect("Failed to create database pool");

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
            .app_data(Data::new(pool.clone()))
            .app_data(query_cfg.clone())
            .app_data(path_cfg.clone())
            .app_data(json_cfg.clone())
            .service(api::jobs::new_job)
            .service(api::jobs::get_jobs)
            .service(api::jobs::get_job)
            .service(api::jobs::rejudge_job)
            .service(api::users::update_user)
            .service(api::users::get_users)
            .service(api::contests::update_contest)
            .service(api::contests::get_contests)
            .service(api::contests::get_contest)
            .service(api::contests::get_rank_list)
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

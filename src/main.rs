use std::{
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

use actix_web::{
    middleware::Logger,
    post,
    web::{Data, JsonConfig, PathConfig, QueryConfig},
    App, HttpServer, Responder,
};
use clap::Parser;
use diesel::{
    connection::SimpleConnection,
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
use r2d2::CustomizeConnection;

type DbPool = Pool<ConnectionManager<SqliteConnection>>;

const DB_URL: &str = "oj.db";
const DB_BUSY_TIMEOUT: &str = "PRAGMA busy_timeout = 30000";
const MQ_URL: &str = "amqp://localhost:5672";
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

// DO NOT REMOVE: used in automatic testing
#[post("/internal/exit")]
#[allow(unreachable_code)]
async fn exit() -> impl Responder {
    log::info!("Shutdown as requested");
    std::process::exit(0);
    "Exited".to_string()
}

#[derive(Debug)]
pub struct ConnectionOption;

// Set busy timeout to avoid conflict writes to the database
impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for ConnectionOption {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute(DB_BUSY_TIMEOUT)
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let args = Args::parse();
    let (config_path, config) = args.config.clone();

    // Independent judger process
    if let Some(id) = args.judger {
        judge::main(id, config);
        return Ok(());
    }

    // Connection to the RabbitMQ server
    let amqp_connection = Arc::new(Mutex::new(
        amiquip::Connection::insecure_open(MQ_URL).expect("Failed to connect to RabbitMQ server"),
    ));

    // Delete existing database
    if args.flush_data {
        log::info!("Flushing persistent data");
        // It's ok that the database doesn't exist
        let _ = std::fs::remove_file(DB_URL);
    }

    // Run migrations
    SqliteConnection::establish(DB_URL)
        .expect("Failed to establish database connection")
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Start some independent judger process
    let judger_count = thread::available_parallelism()
        .expect("Failed to get available parallelism")
        .get();
    let mut judgers = vec![];
    for i in 0..judger_count {
        let judger = Command::new("target/debug/oj")
            .arg("-j")
            .arg(i.to_string())
            .arg("-c")
            .arg(&config_path)
            .spawn()
            .expect("Failed to spawn judger process");
        judgers.push(judger);
    }

    // Create connection pool
    let manager = ConnectionManager::<SqliteConnection>::new(DB_URL);
    let pool = Pool::builder()
        .max_size(16)
        .connection_customizer(Box::new(ConnectionOption))
        .build(manager)
        .expect("Failed to create connection pool");

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
            .app_data(Data::new(
                amqp_connection
                    .lock()
                    .expect("Failed to obtain amqp connection lock")
                    .open_channel(None)
                    .expect("Failed to open amqp channel"),
            ))
            .app_data(query_cfg.clone())
            .app_data(path_cfg.clone())
            .app_data(json_cfg.clone())
            .service(api::jobs::new_job)
            .service(api::jobs::get_jobs)
            .service(api::jobs::get_job)
            .service(api::jobs::rejudge_job)
            .service(api::jobs::cancel_job)
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
        args.config.1.server.bind_address.to_string(),
        args.config.1.server.bind_port,
    ))?
    .run()
    .await?;

    // Kill child processes before exiting
    for judger in &mut judgers {
        let _ = judger.kill();
    }

    Ok(())
}

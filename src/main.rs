use std::{
    env,
    process::{self, Command},
    sync::{Arc, Mutex},
    thread,
};

#[cfg(feature = "authorization")]
use actix_jwt_auth_middleware::{AuthService, Authority};
#[cfg(feature = "authorization")]
use actix_web::web;
use actix_web::{
    middleware::Logger,
    post,
    web::{Data, JsonConfig, PathConfig, QueryConfig},
    App, HttpServer, Responder,
};
#[cfg(feature = "authorization")]
use authorization::UserClaims;
use clap::Parser;
use diesel::{
    connection::SimpleConnection,
    r2d2::{ConnectionManager, Pool},
    Connection, SqliteConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

mod api;
mod authorization;
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
        judge::main(args.parent.unwrap(), id, config);
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
        let judger = Command::new(env::args().next().unwrap())
            .arg("-j")
            .arg(i.to_string())
            .arg("-c")
            .arg(&config_path)
            .arg("-p")
            .arg(&process::id().to_string())
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

    // JWT authority middleware
    #[cfg(feature = "authorization")]
    let auth_authority = Authority::<UserClaims>::default();

    #[cfg(feature = "authorization")]
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(Data::new(config.clone()))
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(auth_authority.clone()))
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
            // Services that can be accessed without authorization
            .service(authorization::register)
            .service(authorization::login)
            // Services that needed to login first
            .service(
                web::scope("")
                    .wrap(AuthService::new(
                        auth_authority.clone(),
                        authorization::verify_service_request_user,
                    ))
                    .service(authorization::change_password)
                    .service(api::jobs::new_job)
                    .service(api::jobs::get_jobs)
                    .service(api::jobs::get_job)
                    .service(api::users::get_users)
                    .service(api::contests::get_contests)
                    .service(api::contests::get_contest)
                    .service(api::contests::get_rank_list)
                    // Services that only author or admin can access
                    .service(api::jobs::rejudge_job)
                    .service(api::jobs::cancel_job)
                    .service(api::contests::update_contest)
                    // Services that only admin can access
                    .service(authorization::privilege)
                    .service(api::users::update_user),
            )
            // DO NOT REMOVE: used in automatic testing
            .service(exit)
    })
    .bind((
        args.config.1.server.bind_address.to_string(),
        args.config.1.server.bind_port,
    ))?
    .run()
    .await?;

    // For automatic test
    #[cfg(not(feature = "authorization"))]
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

use std::fmt::{self, Display};

use actix_jwt_auth_middleware::AuthError;
use actix_web::{error::BlockingError, HttpResponse, ResponseError};
use http::StatusCode;
use serde::Serialize;

/// Error reason
#[derive(Clone, Debug, Serialize)]
#[serde(into = "String")]
pub enum Reason {
    InvalidArgument,
    InvalidState,
    NotFound,
    RateLimit,
    External,
    Internal,
    Forbidden,
}

impl From<Reason> for String {
    fn from(err: Reason) -> Self {
        "ERR_".to_string()
            + match err {
                Reason::InvalidArgument => "INVALID_ARGUMENT",
                Reason::InvalidState => "INVALID_STATE",
                Reason::NotFound => "NOT_FOUND",
                Reason::RateLimit => "RATE_LIMIT",
                Reason::External => "EXTERNAL",
                Reason::Internal => "INTERNAL",
                Reason::Forbidden => "FORBIDDEN",
            }
    }
}

/// An Error
#[derive(Debug, Serialize)]
pub struct Error {
    code: u32,
    pub reason: Reason,
    message: String,
}

impl Error {
    /// Create a new error
    pub fn new(reason: Reason, message: String) -> Self {
        let code = match reason {
            Reason::InvalidArgument => 1,
            Reason::InvalidState => 2,
            Reason::NotFound => 3,
            Reason::RateLimit => 4,
            Reason::External => 5,
            Reason::Internal => 6,
            Reason::Forbidden => 7,
        };
        Error {
            code,
            reason,
            message,
        }
    }
}

impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::NotFound => {
                log::info!(target: "persistent", "Not Found");
                Error::new(Reason::NotFound, "Not Found".to_string())
            }
            diesel::result::Error::DatabaseError(kind, info) => {
                log::error!(
                    "Database error! {kind:?}: {:?} {:?} {:?} {} {:?}",
                    info.table_name(),
                    info.column_name(),
                    info.constraint_name(),
                    info.message(),
                    info.details()
                );
                Error::new(Reason::External, "Database error".to_string())
            }
            diesel::result::Error::AlreadyInTransaction => {
                log::error!("Already In Transaction");
                Error::new(Reason::External, "Database error".to_string())
            }
            diesel::result::Error::BrokenTransactionManager => {
                log::error!("Already In Transaction");
                Error::new(Reason::External, "Database error".to_string())
            }
            _ => {
                log::error!(target: "persistent", "Database error: {}", err);
                Error::new(Reason::External, "Database error".to_string())
            }
        }
    }
}

impl From<r2d2::Error> for Error {
    fn from(err: r2d2::Error) -> Self {
        log::error!(target: "persistent", "Connection pool error: {}", err);
        Error::new(Reason::External, "Database error".to_string())
    }
}

impl From<BlockingError> for Error {
    fn from(err: BlockingError) -> Self {
        log::error!(target: "persistent", "Blocking error: {}", err);
        Error::new(Reason::External, "Database error".to_string())
    }
}

impl From<AuthError> for Error {
    fn from(err: AuthError) -> Self {
        log::error!(target: "auth", "Auth error: {}", err);
        Error::new(Reason::Internal, "Authorization error".to_string())
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

// To generate JSON response from Error
impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self.reason {
            Reason::InvalidArgument => StatusCode::BAD_REQUEST,
            Reason::InvalidState => StatusCode::BAD_REQUEST,
            Reason::NotFound => StatusCode::NOT_FOUND,
            Reason::RateLimit => StatusCode::BAD_REQUEST,
            Reason::External => StatusCode::INTERNAL_SERVER_ERROR,
            Reason::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Reason::Forbidden => StatusCode::FORBIDDEN,
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code()).json(self)
    }
}

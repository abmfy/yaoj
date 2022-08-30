use std::fmt::{self, Display};

use actix_web::{HttpResponse, ResponseError};
use http::StatusCode;
use serde::Serialize;

/// Error reason
#[derive(Debug, Serialize)]
pub enum Reason {
    InvalidArgument,
    InvalidState,
    NotFound,
    RateLimit,
    External,
    Internal,
}

impl ToString for Reason {
    fn to_string(&self) -> String {
        "ERR_".to_string()
            + match self {
                Reason::InvalidArgument => "INVALID_ARGUMENT",
                Reason::InvalidState => "INVALID_STATE",
                Reason::NotFound => "NOT_FOUND",
                Reason::RateLimit => "RATE_LIMIT",
                Reason::External => "EXTERNAL",
                Reason::Internal => "INTERNAL",
            }
    }
}

/// An Error
#[derive(Debug, Serialize)]
pub struct Error {
    code: u32,
    reason: Reason,
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
        };
        Error {
            code,
            reason,
            message,
        }
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
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code()).json(self)
    }
}

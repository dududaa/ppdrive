use std::{env::VarError, fmt::Display, string::FromUtf8Error};

use axum::{extract::multipart::MultipartError, http::StatusCode, response::IntoResponse};
use ppd_shared::errors::Error as SharedError;
use ppd_bk::Error as DBError;
use handlers::errors::HandlerError;

#[derive(Debug)]
pub enum ServerError {
    InitError(String),
    InternalError(String),
    CommonError(SharedError),
    DBError(DBError),
    AuthorizationError(String),
    IOError(String),
    NotFound(String),
    PermissionDenied(String),
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::InitError(msg) => write!(f, "{msg}"),
            ServerError::InternalError(msg) => write!(f, "{msg}"),
            // ServerError::FsError(err) => write!(f, "{err}"),
            ServerError::CommonError(err) => write!(f, "{err}"),
            ServerError::DBError(err) => write!(f, "{err}"),
            ServerError::AuthorizationError(msg) => write!(f, "{msg}"),
            ServerError::IOError(msg) => write!(f, "{msg}"),
            ServerError::NotFound(msg) => write!(f, "{msg}"),
            ServerError::PermissionDenied(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<VarError> for ServerError {
    fn from(value: VarError) -> Self {
        ServerError::InternalError(value.to_string())
    }
}

impl From<HandlerError> for ServerError {
    fn from(value: HandlerError) -> Self {
        ServerError::InternalError(value.to_string())
    }
}

impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        ServerError::IOError(value.to_string())
    }
}

impl From<MultipartError> for ServerError {
    fn from(value: MultipartError) -> Self {
        ServerError::InternalError(value.to_string())
    }
}

impl From<SharedError> for ServerError {
    fn from(value: SharedError) -> Self {
        ServerError::CommonError(value)
    }
}

impl From<DBError> for ServerError {
    fn from(value: DBError) -> Self {
        ServerError::DBError(value)
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            ServerError::AuthorizationError(msg) => (StatusCode::UNAUTHORIZED, msg),
            ServerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ServerError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        resp.into_response()
    }
}

impl From<FromUtf8Error> for ServerError {
    fn from(value: FromUtf8Error) -> Self {
        ServerError::InitError(value.to_string())
    }
}

use std::{env::VarError, fmt::Display, string::FromUtf8Error};

use axum::{extract::multipart::MultipartError, http::StatusCode, response::IntoResponse};
use ppdrive_fs::errors::CoreError;

#[derive(Debug)]
pub enum RestError {
    InitError(String),
    InternalServerError(String),
    CoreError(CoreError),
    AuthorizationError(String),
    IOError(String),
    NotFound(String),
    PermissionDenied(String),
}

impl Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestError::InitError(msg) => write!(f, "{msg}"),
            RestError::InternalServerError(msg) => write!(f, "{msg}"),
            RestError::CoreError(msg) => write!(f, "{msg}"),
            RestError::AuthorizationError(msg) => write!(f, "{msg}"),
            RestError::IOError(msg) => write!(f, "{msg}"),
            RestError::NotFound(msg) => write!(f, "{msg}"),
            RestError::PermissionDenied(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<VarError> for RestError {
    fn from(value: VarError) -> Self {
        RestError::InternalServerError(value.to_string())
    }
}

impl From<serde_json::Error> for RestError {
    fn from(value: serde_json::Error) -> Self {
        RestError::InternalServerError(value.to_string())
    }
}

impl From<std::io::Error> for RestError {
    fn from(value: std::io::Error) -> Self {
        RestError::IOError(value.to_string())
    }
}

impl From<MultipartError> for RestError {
    fn from(value: MultipartError) -> Self {
        RestError::InternalServerError(value.to_string())
    }
}

impl From<CoreError> for RestError {
    fn from(value: CoreError) -> Self {
        RestError::CoreError(value)
    }
}

impl IntoResponse for RestError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            RestError::AuthorizationError(msg) => (StatusCode::UNAUTHORIZED, msg),
            RestError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            RestError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        resp.into_response()
    }
}

impl From<FromUtf8Error> for RestError {
    fn from(value: FromUtf8Error) -> Self {
        RestError::InitError(value.to_string())
    }
}

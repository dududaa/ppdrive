use std::{env::VarError, fmt::Display, string::FromUtf8Error};

use axum::{http::StatusCode, response::IntoResponse};
use ppd_fs::errors::Error as FsError;
use ppd_shared::errors::Error as SharedError;
use ppd_bk::Error as DBError;
use client_tools::errors::Error as ClientError;

#[derive(Debug)]
pub enum HandlerError {
    InitError(String),
    InternalError(String),
    FsError(FsError),
    CommonError(SharedError),
    DBError(DBError),
    AuthorizationError(String),
    IOError(String),
    NotFound(String),
    PermissionDenied(String),
}

impl Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandlerError::InitError(msg) => write!(f, "{msg}"),
            HandlerError::InternalError(msg) => write!(f, "{msg}"),
            HandlerError::FsError(err) => write!(f, "{err}"),
            HandlerError::CommonError(err) => write!(f, "{err}"),
            HandlerError::DBError(err) => write!(f, "{err}"),
            HandlerError::AuthorizationError(msg) => write!(f, "{msg}"),
            HandlerError::IOError(msg) => write!(f, "{msg}"),
            HandlerError::NotFound(msg) => write!(f, "{msg}"),
            HandlerError::PermissionDenied(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<VarError> for HandlerError {
    fn from(value: VarError) -> Self {
        HandlerError::InternalError(value.to_string())
    }
}

impl From<std::io::Error> for HandlerError {
    fn from(value: std::io::Error) -> Self {
        HandlerError::IOError(value.to_string())
    }
}

// impl From<MultipartError> for HandlerError {
//     fn from(value: MultipartError) -> Self {
//         HandlerError::InternalError(value.to_string())
//     }
// }

impl From<FsError> for HandlerError {
    fn from(value: FsError) -> Self {
        HandlerError::FsError(value)
    }
}

impl From<SharedError> for HandlerError {
    fn from(value: SharedError) -> Self {
        HandlerError::CommonError(value)
    }
}

impl From<DBError> for HandlerError {
    fn from(value: DBError) -> Self {
        HandlerError::DBError(value)
    }
}

impl From<ClientError> for HandlerError {
    fn from(value: ClientError) -> Self {
        HandlerError::InternalError(value.to_string())
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            HandlerError::AuthorizationError(msg) => (StatusCode::UNAUTHORIZED, msg),
            HandlerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            HandlerError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        resp.into_response()
    }
}

impl From<FromUtf8Error> for HandlerError {
    fn from(value: FromUtf8Error) -> Self {
        HandlerError::InitError(value.to_string())
    }
}

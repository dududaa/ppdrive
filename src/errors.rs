use std::{env::VarError, fmt::Display};

use axum::response::IntoResponse;
use diesel_async::pooled_connection::bb8::RunError;
use reqwest::StatusCode;

#[derive(Debug)]
pub enum PPDriveError {
    InitError(String),
    InternalServerError(String),
    DatabaseError(String)
}

impl Display for PPDriveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PPDriveError::InitError(msg) => write!(f, "{msg}"),
            PPDriveError::InternalServerError(msg) => write!(f, "{msg}"),
            PPDriveError::DatabaseError(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<VarError> for PPDriveError {
    fn from(value: VarError) -> Self {
        PPDriveError::InternalServerError(value.to_string())
    }
}

impl From<RunError> for PPDriveError {
    fn from(value: RunError) -> Self {
        PPDriveError::DatabaseError(value.to_string())
    }
}

impl IntoResponse for PPDriveError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
        };

        resp.into_response()
    }
}
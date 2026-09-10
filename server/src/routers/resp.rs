//! Standardised JSON response and error types.
//!
//! [`ApiResponse<T>`] is the handler return type; [`ResponseError`] converts
//! internal errors into safe, non-leaking HTTP responses.

use std::fmt::Display;
use std::io::Error;
use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::io;

/// Standard handler return type: either a success payload or a [`ResponseError`].
pub type ApiResponse<T> = Result<ResponsePayload<T>, ResponseError>;

pub struct ResponsePayload<T: Serialize> {
    data: T,
    status_code: StatusCode,
}

impl<T: Serialize> ResponsePayload<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            status_code: StatusCode::OK,
        }
    }

    pub fn with_status_code(mut self, status_code: StatusCode) -> Self {
        self.status_code = status_code;
        self
    }

    #[allow(dead_code)]
    pub fn data(&self) -> &T {
        &self.data
    }
}

impl<T: Serialize> IntoResponse for ResponsePayload<T> {
    fn into_response(self) -> Response {
        let body = (self.status_code, Json(self.data));
        body.into_response()
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

pub struct ResponseError {
    message: String,
    status_code: StatusCode,
}

impl ResponseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn with_status_code(mut self, status_code: StatusCode) -> Self {
        self.status_code = status_code;
        self
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
        };
        (self.status_code, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ResponseError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!("internal error: {err:#}");
        api_error("internal server error")
    }
}

impl From<io::Error> for ResponseError {
    fn from(err: Error) -> Self {
        tracing::error!("io error: {err}");
        api_error("internal server error")
    }
}

impl From<sqlx::Error> for ResponseError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => api_error("resource not found")
                .with_status_code(StatusCode::NOT_FOUND),
            _ => {
                tracing::error!("database error: {err}");
                api_error("internal server error")
            }
        }
    }
}

/// Build a [`ResponseError`] with the given display message (defaults to 500).
pub fn api_error(message: impl Display) -> ResponseError {
    ResponseError::new(message.to_string())
}

/// Wrap `data` into a 200 OK [`ResponsePayload`].
pub fn api_response<T: Serialize>(data: T) -> Result<ResponsePayload<T>, ResponseError> {
    Ok(ResponsePayload::new(data))
}
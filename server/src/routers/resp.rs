use std::fmt::Display;
use std::io::Error;
use axum::http::{header, HeaderValue, StatusCode};
use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tokio::io;

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
        (
            self.status_code,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            )],
            self.message,
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ResponseError {
    fn from(err: anyhow::Error) -> Self {
        api_error(err)
    }
}

impl From<io::Error> for ResponseError {
    fn from(value: Error) -> Self {
        api_error(value)
    }
}

impl From<sqlx::Error> for ResponseError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => api_error(err).with_status_code(StatusCode::NOT_FOUND),
            _ => api_error(err),
        }
    }
}

pub fn api_error(message: impl Display) -> ResponseError {
    ResponseError::new(message.to_string())
}

pub fn api_response<T: Serialize>(data: T) -> Result<ResponsePayload<T>, ResponseError> {
    Ok(ResponsePayload::new(data))
}
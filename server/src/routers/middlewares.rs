//! Axum extractors for authentication and session verification.
//!
//! [`ClientExtractor`] validates the client API key header;
//! [`UploadMiddleware`] verifies the signed upload-session payload;
//! [`DownloadMiddleware`] verifies the signed download token.

use crate::routers::resp::{ResponseError, api_error};
use crate::state::AppState;
use axum::extract::{FromRef, FromRequestParts, Path};
use axum::http::StatusCode;
use axum::http::request::Parts;
use shared::db::client::verify_client;
use shared::server::{DownloadInfo, UploadInfo};
use shared::hasher::errors::PayloadVerificationError;

/// Axum extractor that authenticates the request via the client API-key header.
pub struct ClientExtractor(i32);
impl ClientExtractor {
    pub fn id(&self) -> i32 {
        self.0
    }
}

impl<S> FromRequestParts<S> for ClientExtractor
where
    S: Send + Sync + Clone + 'static,
    AppState: FromRef<S>,
{
    type Rejection = ResponseError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        let header_key = state.config().client_header_key.clone();
        let header = parts.headers.get(&header_key).ok_or(
            api_error("missing client header key").with_status_code(StatusCode::UNAUTHORIZED),
        )?;

        let client_token = header
            .to_str()
            .map_err(|_| api_error("invalid client token"))?;

        let client_id = verify_client(state.db(), state.secrets(), client_token)
            .await
            .map_err(|e| {
                tracing::error!("client verification failed: {e}");
                api_error("client verification failed")
                    .with_status_code(StatusCode::UNAUTHORIZED)
            })?;

        Ok(Self(client_id))
    }
}

/// Axum extractor that verifies the signed upload session token from the URL path.
pub struct UploadMiddleware(pub UploadInfo);

impl<S> FromRequestParts<S> for UploadMiddleware
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ResponseError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(payload) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|e| api_error(e))?;

        let state = AppState::from_ref(state);
        match UploadInfo::verify(&payload, state.db(), state.secrets(), state.hasher()).await {
            Ok(info) => Ok(Self(info)),
            Err(err) => {
                let resp = match err {
                    PayloadVerificationError::Error(err) => api_error(err),
                    PayloadVerificationError::Expired => api_error("session expired"),
                };

                Err(resp.with_status_code(StatusCode::UNAUTHORIZED))
            }
        }
    }
}

/// Axum extractor that verifies the signed download token from the URL path.
pub struct DownloadMiddleware(pub DownloadInfo);

impl<S> FromRequestParts<S> for DownloadMiddleware
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ResponseError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(token) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|e| api_error(e))?;

        let state = AppState::from_ref(state);
        match DownloadInfo::verify(&token, state.db(), state.secrets(), state.hasher()).await {
            Ok(info) => Ok(Self(info)),
            Err(err) => {
                let resp = match err {
                    PayloadVerificationError::Error(err) => api_error(err),
                    PayloadVerificationError::Expired => api_error("download token expired"),
                };

                Err(resp.with_status_code(StatusCode::UNAUTHORIZED))
            }
        }
    }
}

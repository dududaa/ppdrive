//! Axum extractors for authentication and session verification.
//!
//! [`ClientExtractor`] validates the client API key header;
//! [`UserExtractor`] validates the user JWT token from the Authorization header;
//! [`UploadMiddleware`] verifies the signed upload-session payload;
//! [`DownloadMiddleware`] verifies the signed download token.

use crate::routers::resp::{ResponseError, api_error};
use crate::state::AppState;
use axum::extract::{FromRef, FromRequestParts, Path};
use axum::http::StatusCode;
use axum::http::request::Parts;
use shared::db::client::verify_client;
use shared::server::{DownloadInfo, UserInfo, UploadInfo};
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
            .map_err(|_| api_error("invalid client token").with_status_code(StatusCode::BAD_REQUEST))?;

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

/// Axum extractor that authenticates the request via a user JWT token in the Authorization header.
pub struct UserExtractor(pub UserInfo);
impl UserExtractor {
    pub fn email(&self) -> &str {
        &self.0.user_email
    }
}

impl<S> FromRequestParts<S> for UserExtractor
where
    S: Send + Sync + Clone + 'static,
    AppState: FromRef<S>,
{
    type Rejection = ResponseError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        let auth_header = parts.headers.get("authorization").ok_or(
            api_error("missing authorization header").with_status_code(StatusCode::UNAUTHORIZED),
        )?;

        let token = auth_header
            .to_str()
            .map_err(|_| api_error("invalid authorization header").with_status_code(StatusCode::BAD_REQUEST))?
            .strip_prefix("Bearer ")
            .ok_or_else(|| api_error("invalid authorization format, expected 'Bearer <token>'")
                .with_status_code(StatusCode::UNAUTHORIZED))?;

        let info = UserInfo::verify(token, state.db(), state.secrets(), state.hasher())
            .await
            .map_err(|e| {
                let resp = match e {
                    PayloadVerificationError::Expired => api_error("token expired"),
                    PayloadVerificationError::Error(err) => {
                        tracing::error!("user verification failed: {err}");
                        api_error("invalid token")
                    }
                };
                resp.with_status_code(StatusCode::UNAUTHORIZED)
            })?;

        Ok(Self(info))
    }
}

/// Combined auth extractor that accepts either a client API key or a user Bearer token.
///
/// Checks for the client header first, then falls back to the Authorization Bearer token.
pub enum AuthExtractor {
    Client(ClientExtractor),
    User(UserExtractor),
}

impl AuthExtractor {
    #[allow(dead_code)]
    pub fn client(&self) -> Option<&ClientExtractor> {
        match self {
            AuthExtractor::Client(c) => Some(c),
            AuthExtractor::User(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn user(&self) -> Option<&UserExtractor> {
        match self {
            AuthExtractor::Client(_) => None,
            AuthExtractor::User(u) => Some(u),
        }
    }
}

impl<S> FromRequestParts<S> for AuthExtractor
where
    S: Send + Sync + Clone + 'static,
    AppState: FromRef<S>,
{
    type Rejection = ResponseError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        // Try client header first
        let header_key = state.config().client_header_key.clone();
        if let Some(header) = parts.headers.get(&header_key)
            && let Ok(client_token) = header.to_str()
            && let Ok(client_id) = verify_client(state.db(), state.secrets(), client_token).await
        {
            return Ok(AuthExtractor::Client(ClientExtractor(client_id)));
        }

        // Fall back to Authorization Bearer token
        if let Some(auth_header) = parts.headers.get("authorization")
            && let Ok(auth_str) = auth_header.to_str()
            && let Some(token) = auth_str.strip_prefix("Bearer ")
        {
            match UserInfo::verify(token, state.db(), state.secrets(), state.hasher()).await {
                Ok(info) => return Ok(AuthExtractor::User(UserExtractor(info))),
                Err(PayloadVerificationError::Expired) => {
                    return Err(api_error("token expired")
                        .with_status_code(StatusCode::UNAUTHORIZED));
                }
                Err(PayloadVerificationError::Error(err)) => {
                    tracing::error!("user verification failed: {err}");
                    return Err(api_error("invalid token")
                        .with_status_code(StatusCode::UNAUTHORIZED));
                }
            }
        }

        Err(api_error("authentication required")
            .with_status_code(StatusCode::UNAUTHORIZED))
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
            .map_err(api_error)?;

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
            .map_err(api_error)?;

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

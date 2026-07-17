use crate::routers::resp::{ResponseError, api_error};
use crate::state::AppState;
use axum::extract::{FromRef, FromRequestParts, Path};
use axum::http::StatusCode;
use axum::http::request::Parts;
use shared::client::verify_client;
use shared::server::UploadInfo;
use shared::server::errors::PayloadVerificationError;

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
            .map_err(|e| api_error(format!("client verification failed: {e}")))?;

        Ok(Self(client_id))
    }
}

pub struct UploadMiddleware(pub UploadInfo);

impl UploadMiddleware {
    pub fn info(&self) -> &UploadInfo {
        &self.0
    }
}

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
        match UploadInfo::verify(&payload, state.db()).await {
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

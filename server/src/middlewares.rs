use crate::resp::{api_error, ResponseError};
use crate::state::AppState;
use axum::extract::{FromRef, FromRequestParts};
use shared::client::verify_client;

pub struct ClientMiddleware(i32);
impl ClientMiddleware {
    pub fn id(&self) -> i32 {
        self.0
    }
}

impl<S> FromRequestParts<S> for ClientMiddleware
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = ResponseError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);

        let header_key = state.config().client_header_key.clone();
        let header = parts
            .headers
            .get(&header_key)
            .ok_or(api_error("missing client header key"))?;

        let client_token = header
            .to_str()
            .map_err(|_| api_error("invalid client token"))?;

        let client_id = verify_client(state.pool(), state.secrets(), client_token)
            .await
            .map_err(|e| api_error(format!("client verification failed: {e}")))?;

        Ok(Self(client_id))
    }
}

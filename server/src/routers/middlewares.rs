use std::ops::Deref;
use crate::routers::resp::{ResponseError, api_error};
use crate::state::AppState;
use crate::utils::{Claims, decode_jwt, ClaimsData};
use axum::extract::{FromRef, FromRequestParts};
use axum::http::{StatusCode, header};
use shared::client::verify_client;

pub struct ClientExtractor(i32);
impl ClientExtractor {
    pub fn id(&self) -> i32 {
        self.0
    }
}

impl<S> FromRequestParts<S> for ClientExtractor
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
        let header = parts.headers.get(&header_key).ok_or(
            api_error("missing client header key").with_status_code(StatusCode::UNAUTHORIZED),
        )?;

        let client_token = header
            .to_str()
            .map_err(|_| api_error("invalid client token"))?;

        let client_id = verify_client(state.pool(), state.secrets(), client_token)
            .await
            .map_err(|e| api_error(format!("client verification failed: {e}")))?;

        Ok(Self(client_id))
    }
}

pub struct ClaimsExtractor(Claims);
impl<S> FromRequestParts<S> for ClaimsExtractor
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
        let header = parts.headers.get(header::AUTHORIZATION).ok_or(
            api_error("missing authorization header.").with_status_code(StatusCode::UNAUTHORIZED),
        )?;

        let auth_str = header
            .to_str()
            .map_err(|_| api_error("invalid authorization header"))?;

        let bearer = "Bearer "; // TODO: Make this configurable.
        if !auth_str.starts_with(bearer) {
            return Err(api_error("invalid authorization header"));
        }

        let auth_split = bearer
            .split_once(' ')
            .ok_or(api_error("invalid authorization header"))?;

        let claims = decode_jwt(state.secrets(), auth_split.1.trim())?;

        Ok(Self(claims))
    }
}

impl Deref for ClaimsExtractor {
    type Target = Claims;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
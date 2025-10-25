use std::ops::Deref;

use crate::tools::verify_client;
use crate::{HandlerResult, errors::HandlerError};
use crate::{jwt::decode_jwt, prelude::state::HandlerState};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{HeaderValue, header::AUTHORIZATION, request::Parts},
};
use ppd_bk::models::user::Users;
use ppd_shared::opts::ServiceConfig;

/// A middleware that accepts client token, validates it and return the client's id
pub struct ClientExtractor(u64);

impl ClientExtractor {
    pub fn id(&self) -> &u64 {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientExtractor
where
    HandlerState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = HandlerError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_key =
            parts
                .headers
                .get("ppd-client-token")
                .ok_or(HandlerError::AuthorizationError(
                    "missing 'ppd-client-token' in headers".to_string(),
                ))?;

        let token = client_key
            .to_str()
            .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

        let state = HandlerState::from_ref(state);
        let secrets = state.secrets();

        let id = verify_client(state.db(), secrets.deref(), token)
            .await
            .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

        Ok(ClientExtractor(id))
    }
}

/// This middleware checks if a given user is created by the client and returns the user id.
/// WARNING: This may not be as performant as [UserExtractor] because it uses database for 
/// validation on every request.
pub struct ClientUserExtractor {
    id: u64
}

impl ClientUserExtractor {
    pub fn id(&self) -> &u64 {
        &self.id
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientUserExtractor
where
    HandlerState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = HandlerError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let part_clone = parts.clone();
        let user_id_xh =
            part_clone
                .headers
                .get("ppd-client-user")
                .ok_or(HandlerError::AuthorizationError(
                    "missing 'ppd-client-user' in headers".to_string(),
                ))?;

        let user_id = user_id_xh
            .to_str()
            .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

        let client = ClientExtractor::from_request_parts(parts, state).await?;
        let state = HandlerState::from_ref(state);

        let db = state.db();
        let user = Users::get_for_client(db, user_id, client.id())
            .await
            .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?
            .ok_or(HandlerError::AuthorizationError(
                "user with provided id does not exist or may not be accessible by client"
                    .to_string(),
            ))?;

        Ok(ClientUserExtractor{id: user.id()})
    }
}

/// An extractor that accepts authorization token, verifies the token and returns user id.
pub struct UserExtractor(u64);
impl UserExtractor {
    pub fn id(&self) -> &u64 {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for UserExtractor
where
    HandlerState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = HandlerError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match parts.headers.get(AUTHORIZATION) {
            Some(auth) => {
                let state = HandlerState::from_ref(state);
                let config = state.config();

                match &config.auth.url {
                    Some(_url) => {
                        unimplemented!("external url feature not implemented.")
                    }
                    None => {
                        let user = get_local_user(&state, auth, &config).await?;
                        Ok(UserExtractor(user))
                    }
                }
            }
            None => Err(HandlerError::AuthorizationError(
                "authorization header required".to_string(),
            )),
        }
    }
}

async fn get_local_user(
    state: &HandlerState,
    header: &HeaderValue,
    config: &ServiceConfig,
) -> HandlerResult<u64> {
    let secrets = state.secrets();
    let claims = decode_jwt(header, secrets.jwt_secret(), config)?;

    Ok(claims.sub)
}

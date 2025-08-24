use std::ops::Deref;

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts, HeaderValue},
};
use client_tools::verify_client;
use crate::{errors::HandlerError, state::HandlerState, jwt::decode_jwt, HandlerResult};


pub struct RequestUser {
    id: u64,
}

impl RequestUser {
    pub fn id(&self) -> &u64 {
        &self.id
    }
}

/// A user that client is making request for. This extractor MUST be added
/// after [ClientRoute] to ensure that the client is valid.
pub struct ClientUser(pub RequestUser);

#[async_trait]
impl<S> FromRequestParts<S> for ClientUser
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

                match config.auth().url() {
                    Some(_url) => {
                        unimplemented!("external url feature not implemented.")
                    }
                    None => {
                        let user = get_local_user(&state, &auth).await?;
                        Ok(ClientUser(user))
                    }
                }
            }
            None => Err(HandlerError::AuthorizationError(
                "authorization header required".to_string(),
            )),
        }
    }
}

pub struct ClientRoute(u64);

impl ClientRoute {
    pub fn id(&self) -> &u64 {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientRoute
where
    HandlerState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = HandlerError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_key = parts.headers.get("x-ppd-client");
        let state = HandlerState::from_ref(state);

        match client_key {
            Some(key) => {
                let token = key
                    .to_str()
                    .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

                let secrets = state.secrets();
                let id = verify_client(state.db(), secrets.deref(), token).await.map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

                Ok(ClientRoute(id))
            }
            _ => Err(HandlerError::AuthorizationError(
                "missing 'x-client-key' headers".to_string(),
            )),
        }
    }
}

async fn get_local_user(state: &HandlerState, header: &HeaderValue) -> HandlerResult<RequestUser> {
    let secrets = state.secrets();
    // let db = state.db();

    let claims = decode_jwt(header, secrets.jwt_secret())?;
    // let user = Users::get(db, &claims.sub).await?;
    // let id = user.id().to_owned();

    Ok(RequestUser { id: claims.sub })
}

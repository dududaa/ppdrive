use std::ops::Deref;

use crate::tools::verify_client;
use crate::{HandlerResult, errors::HandlerError};
use crate::{jwt::decode_jwt, prelude::state::HandlerState};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{HeaderValue, header::AUTHORIZATION, request::Parts},
};
use ppd_shared::opts::ServiceConfig;

pub struct RequestUser {
    id: u64,
}

impl RequestUser {
    pub fn id(&self) -> &u64 {
        &self.id
    }
}

/// An extractor that accepts authorization token, verifies the token and returns user id.
pub struct UserExtractor(pub u64);
impl UserExtractor {
    pub fn id(&self) -> u64 {
        self.0
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
        let client_key = parts.headers.get("ppd-client-token");
        let state = HandlerState::from_ref(state);

        match client_key {
            Some(key) => {
                let token = key
                    .to_str()
                    .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

                let secrets = state.secrets();
                let id = verify_client(state.db(), secrets.deref(), token)
                    .await
                    .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

                Ok(ClientExtractor(id))
            }
            _ => Err(HandlerError::AuthorizationError(
                "missing 'ppd-client-token' in headers".to_string(),
            )),
        }
    }
}

async fn get_local_user(state: &HandlerState, header: &HeaderValue, config: &ServiceConfig) -> HandlerResult<u64> {
    let secrets = state.secrets();
    let claims = decode_jwt(header, secrets.jwt_secret(), config)?;
    
    Ok(claims.sub)
}

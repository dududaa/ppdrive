use std::ops::Deref;

use crate::tools::verify_client;
use crate::{HandlerResult, errors::HandlerError};
use crate::{jwt::decode_jwt, prelude::state::HandlerState};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{HeaderValue, header::AUTHORIZATION, request::Parts},
};
use ppd_bk::RBatis;
use ppd_bk::models::bucket::Buckets;
use ppd_bk::models::user::Users;
use ppd_shared::opts::ServiceConfig;

/// A middleware that accepts client token, validates it and return the client's id
pub struct ClientExtractor {
    id: u64,
    max_bucket_size: Option<f64>
}

impl BucketSizeValidator for ClientExtractor {
    fn id(&self) -> &u64 {
        &self.id
    }
    
    fn max_bucket_size(&self) -> &Option<f64> {
        &self.max_bucket_size
    }

    async fn current_size(&self, db: &RBatis) -> HandlerResult<f64> {
        let size = Buckets::client_total_bucket_size(db, self.id()).await?;
        Ok(size)
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

        let (id, max_bucket_size) = verify_client(state.db(), secrets.deref(), token)
            .await
            .map_err(|err| HandlerError::AuthorizationError(err.to_string()))?;

        Ok(ClientExtractor { id, max_bucket_size })
    }
}

/// This middleware checks if a given user is created by the client and returns the user id.
/// WARNING: This may not be as performant as [UserExtractor] because it queries database for
/// validation on every single request.
pub struct ClientUserExtractor {
    id: u64,
    max_bucket_size: Option<f64>,
}

impl BucketSizeValidator for ClientUserExtractor {
    fn id(&self) -> &u64 {
        &self.id
    }
    
    fn max_bucket_size(&self) -> &Option<f64> {
        &self.max_bucket_size
    }

    async fn current_size(&self, db: &RBatis) -> HandlerResult<f64> {
        let size = Buckets::user_total_bucket_size(db, self.id()).await?;
        Ok(size)
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

        Ok(ClientUserExtractor {
            id: user.id(),
            max_bucket_size: *user.max_bucket_size(),
        })
    }
}

/// An extractor that accepts authorization token, verifies the token and returns user id.
pub struct UserExtractor {
    id: u64,
    max_bucket_size: Option<f64>,
}

impl BucketSizeValidator for UserExtractor {
    fn id(&self) -> &u64 {
        &self.id
    }

    fn max_bucket_size(&self) -> &Option<f64> {
        &self.max_bucket_size
    }

    async fn current_size(&self, db: &RBatis) -> HandlerResult<f64> {
        let size = Buckets::user_total_bucket_size(db, self.id()).await?;
        Ok(size)
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
                        Ok(user)
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
) -> HandlerResult<UserExtractor> {
    let secrets = state.secrets();
    let claims = decode_jwt(header, secrets.jwt_secret(), config)?;

    Ok(UserExtractor { id: *claims.sub(), max_bucket_size: *claims.user_bucket_size() })
}

pub trait BucketSizeValidator {
    fn id(&self) -> &u64;
    fn max_bucket_size(&self) -> &Option<f64>;

    #[allow(async_fn_in_trait)]
    async fn current_size(&self, db: &RBatis) -> HandlerResult<f64>;

    #[allow(async_fn_in_trait)]
    async fn validate_bucket_size(&self, db: &RBatis, bucket_size: &Option<f64>) -> HandlerResult<()> {
        if let Some(max_size) = self.max_bucket_size() {
            let size = bucket_size.ok_or(HandlerError::PermissionError(
                "you must provide \"partition_size\" option for this bucket".to_string(),
            ))?;

            let current_size = self.current_size(db).await?;
            let total_size = current_size + size;

            if total_size > *max_size {
                return Err(HandlerError::PermissionError(
                    "total bucket size for this user is exceeded".to_string(),
                ));
            }
        }

        Ok(())
    }
}

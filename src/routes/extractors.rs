use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{
    errors::AppError,
    models::user::{User, UserRole},
    state::AppState,
    utils::{jwt::decode_jwt, tools::client::verify_client},
};

pub struct CurrentUser {
    id: i32,
    role: UserRole,
}

impl CurrentUser {
    /// Checks if [CurrentUser] can create assets
    pub fn can_create(&self) -> bool {
        matches!(self.role, UserRole::Creator)
    }

    pub fn id(&self) -> &i32 {
        &self.id
    }
}

pub struct ClientRoute;

#[async_trait]
impl<S> FromRequestParts<S> for ClientRoute
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_key = parts.headers.get("x-ppd-client");
        let state = AppState::from_ref(state);

        match client_key {
            Some(key) => {
                let client_id = key
                    .to_str()
                    .map_err(|err| AppError::AuthorizationError(err.to_string()))?;

                let valid = verify_client(&state, client_id).await?;
                if !valid {
                    return Err(AppError::AuthorizationError(
                        "client authorization failed".to_string(),
                    ));
                }

                Ok(ClientRoute)
            }
            _ => Err(AppError::AuthorizationError(
                "missing 'x-client-key' headers".to_string(),
            )),
        }
    }
}

/// A user that client is making request for. This extractor MUST be added
/// after [ClientRoute] to ensure that the client is valid.
pub struct ExtractUser(pub CurrentUser);

#[async_trait]
impl<S> FromRequestParts<S> for ExtractUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match parts.headers.get(AUTHORIZATION) {
            Some(x_user) => {
                let state = AppState::from_ref(state);
                let config = state.config();

                let claims = decode_jwt(x_user, config.jwt_secret())?;
                let user = User::get(&state, &claims.sub).await?;

                let id = user.id().to_owned();
                let role = user.role().clone();
                Ok(ExtractUser(CurrentUser { id, role }))
            }
            None => Err(AppError::AuthorizationError(
                "missing 'x-ppd-user' header".to_string(),
            )),
        }
    }
}

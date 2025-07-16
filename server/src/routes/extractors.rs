use std::ops::Deref;

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{errors::AppError, state::AppState, utils::jwt::decode_jwt};

use ppdrive_core::{
    models::{
        permission::{AssetPermissions, Permission},
        user::{UserRole, Users},
    },
    tools::verify_client,
};

pub struct CurrentUser {
    id: u64,
    role: UserRole,
}

impl CurrentUser {
    /// Checks if [CurrentUser] can create assets
    pub fn can_manage(&self) -> bool {
        !matches!(self.role, UserRole::Basic)
    }

    pub fn id(&self) -> &u64 {
        &self.id
    }

    /// checks if user has read permission for the given asset
    pub async fn can_read_asset(&self, state: &AppState, asset_id: &u64) -> Result<(), AppError> {
        let db = state.db();

        AssetPermissions::exists(db, self.id(), asset_id, Permission::Read).await?;
        Ok(())
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
            Some(auth) => {
                let state = AppState::from_ref(state);
                let config = state.config();

                match config.auth().url() {
                    Some(_url) => {
                        unimplemented!("external url feature not implemented.")
                    }
                    None => {
                        let secrets = state.secrets();
                        let db = state.db();

                        let claims = decode_jwt(auth, secrets.jwt_secret())?;
                        let user = Users::get(db, &claims.sub).await?;
                        let id = user.id().to_owned();

                        let role = user.role()?;

                        Ok(ExtractUser(CurrentUser { id, role }))
                    }
                }
            }
            None => Err(AppError::AuthorizationError(
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
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_key = parts.headers.get("x-ppd-client");
        let state = AppState::from_ref(state);

        match client_key {
            Some(key) => {
                let token = key
                    .to_str()
                    .map_err(|err| AppError::AuthorizationError(err.to_string()))?;

                let secrets = state.secrets();
                let id = verify_client(state.db(), secrets.deref(), token).await?;

                Ok(ClientRoute(id))
            }
            _ => Err(AppError::AuthorizationError(
                "missing 'x-client-key' headers".to_string(),
            )),
        }
    }
}

pub struct ManagerRoute;
#[async_trait]
impl<S> FromRequestParts<S> for ManagerRoute
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user_ext = ExtractUser::from_request_parts(parts, state).await?;
        let user = user_ext.0;

        if !user.can_manage() {
            return Err(AppError::AuthorizationError(
                "user does not have permission to manage".to_string(),
            ));
        }

        Ok(ManagerRoute)
    }
}

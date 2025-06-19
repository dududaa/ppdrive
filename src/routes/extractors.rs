use std::path::Path;

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{
    errors::AppError,
    models::{
        permission::{AssetPermission, Permission},
        user::{User, UserRole},
    },
    state::AppState,
    utils::{fs::check_folder_size, jwt::decode_jwt, tools::client::verify_client},
};

pub struct CurrentUser {
    id: i32,
    role: UserRole,
    partition: Option<String>,
    folder_max_size: Option<i64>,
}

impl CurrentUser {
    /// Checks if [CurrentUser] can create assets
    pub fn can_manage(&self) -> bool {
        !matches!(self.role, UserRole::Basic)
    }

    pub fn id(&self) -> &i32 {
        &self.id
    }

    pub fn folder_max_size(&self) -> &Option<i64> {
        &self.folder_max_size
    }

    pub async fn partition_size(&self) -> Result<Option<u64>, AppError> {
        let mut size = None;
        if let Some(partition) = &self.partition {
            let mut folder_size = 0;

            let dir = Path::new(partition);
            if !dir.exists() {
                tokio::fs::create_dir_all(dir).await?;
                return Ok(Some(folder_size));
            }

            check_folder_size(partition, &mut folder_size).await?;
            size = Some(folder_size)
        }

        Ok(size)
    }

    /// checks if user has read permission for the given asset
    pub async fn can_read_asset(&self, state: &AppState, asset_id: &i32) -> Result<(), AppError> {
        AssetPermission::check(state, self.id(), asset_id, Permission::Read).await
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

                let claims = decode_jwt(auth, config.jwt_secret())?;
                let user = User::get(&state, &claims.sub).await?;
                let id = user.id().to_owned();

                let role = user.role().clone();
                let partition = user.partition().clone();
                let folder_max_size = *user.partition_size();

                Ok(ExtractUser(CurrentUser {
                    id,
                    role,
                    partition,
                    folder_max_size,
                }))
            }
            None => Err(AppError::AuthorizationError(
                "authorization header required".to_string(),
            )),
        }
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

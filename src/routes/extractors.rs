use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use crate::{
    errors::AppError,
    models::{
        permission::{Permission, PermissionGroup},
        user::User,
    },
    state::AppState,
    utils::tools::client::verify_client,
};

pub struct CurrentUser {
    pub id: i32,
    permission_group: PermissionGroup,
    permissions: Option<Vec<Permission>>,
}

impl CurrentUser {
    /// Checks if [CurrentUser] can create assets
    pub fn can_create(&self) -> bool {
        match self.permission_group {
            PermissionGroup::Custom => {
                let d = vec![];
                let perms = self.permissions.as_ref().unwrap_or(&d);
                let find_write = perms.iter().find(|perm| perm.default_write());
                find_write.is_some()
            }
            _ => self.permission_group.default_write(),
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

/// A user that client is making request for. This extractor MUST be added
/// after [ClientRoute] to ensure that the client is valid.
pub struct ClientUser(pub CurrentUser);

#[async_trait]
impl<S> FromRequestParts<S> for ClientUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let x_user = parts.headers.get("x-ppd-user");
        let user = x_user.map(|header| header.to_str().unwrap_or("").to_string());

        match user {
            Some(user_id) => {
                let state = AppState::from_ref(state);
                let user = User::get_by_pid(&state, &user_id).await?;

                let permission_group = PermissionGroup::try_from(*user.permission_group())?;
                let permissions = user.permissions(&state).await?;

                Ok(ClientUser(CurrentUser {
                    id: *user.id(),
                    permission_group,
                    permissions,
                }))
            }
            None => Err(AppError::AuthorizationError(
                "missing 'x-ppd-user' header".to_string(),
            )),
        }
    }
}

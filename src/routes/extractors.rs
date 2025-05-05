use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use reqwest::{header::AUTHORIZATION, StatusCode};
use serde::Deserialize;

use crate::{
    errors::AppError,
    models::{user::User, Permission, PermissionGroup},
    state::AppState,
    utils::verify_client,
};

#[derive(Deserialize)]
pub struct AuthUser {
    id: String,
}

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

pub struct UserExtractor(pub CurrentUser);

#[async_trait]
impl<S> FromRequestParts<S> for UserExtractor
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract app state
        // https://docs.rs/axum/0.6.4/axum/extract/struct.State.html#for-library-authors
        let state = AppState::from_ref(state);
        let config = state.config();

        match parts.headers.get(AUTHORIZATION) {
            Some(auth) => {
                let resp = reqwest::Client::new()
                    .get(config.auth_url())
                    .header(AUTHORIZATION, auth)
                    .send()
                    .await
                    .map_err(|err| {
                        tracing::error!("unable to send auth request: {err}");
                        AppError::AuthorizationError(err.to_string())
                    })?;

                let status = resp.status();
                let c = resp.text().await?;
                if ![StatusCode::OK, StatusCode::CREATED].contains(&status) {
                    tracing::error!("auth error: {c}");
                    return Err(AppError::AuthorizationError(c));
                }

                let auth_user: AuthUser = serde_json::from_str(&c)?;
                let user_id = auth_user.id;

                let user = User::get_by_pid(&state, &user_id).await?;

                let permission_group = PermissionGroup::try_from(*user.permission_group())?;
                let permissions = user.permissions(&state).await?;

                let extractor = UserExtractor(CurrentUser {
                    id: user.id,
                    permission_group,
                    permissions,
                });

                Ok(extractor)
            }
            None => Err(AppError::AuthorizationError(
                "Authorization not provided".to_string(),
            )),
        }
    }
}

pub struct AdminRoute;

#[async_trait]
impl<S> FromRequestParts<S> for AdminRoute
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_id = parts.headers.get("x-client-id");
        let state = AppState::from_ref(state);

        match client_id {
            Some(client_id) => {
                let client_id = client_id
                    .to_str()
                    .map_err(|err| AppError::AuthorizationError(err.to_string()))?;

                let valid = verify_client(&state, client_id).await?;
                if !valid {
                    return Err(AppError::AuthorizationError(
                        "unable to to verify the client.".to_string(),
                    ));
                }

                Ok(AdminRoute)
            }
            _ => Err(AppError::AuthorizationError(
                "missing authentication headers".to_string(),
            )),
        }
    }
}

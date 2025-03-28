use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use reqwest::{header::AUTHORIZATION, StatusCode};
use serde::Deserialize;

use crate::{errors::AppError, models::{user::User, Permission, PermissionGroup}, state::AppState, utils::get_env};

#[derive(Deserialize)]
pub struct AuthUser {
    id: i32,
    username: String,
}

pub struct CurrentUser {
    pub id: i32,
    username: String,
    is_admin: bool,
    permission_group: PermissionGroup,
    permissions: Option<Vec<Permission>>
}

impl CurrentUser {
    /// Checks if [CurrentUser] can create assets
    pub fn can_create(&self) -> bool {
        match self.permission_group {
            PermissionGroup::Custom => {
                let d = vec![];
                let perms = self.permissions.as_ref().unwrap_or(&d);
                let find_write = perms.iter().find(|perm| perm.default_write() );
                find_write.is_some()
            },
            _ => self.permission_group.default_write()
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
        match parts.headers.get(AUTHORIZATION) {
            Some(auth) => {
                let url = get_env("PPDRIVE_AUTH_URL")?;
                let resp = reqwest::Client::new()
                    .get(url)
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

                // Extract app state
                // https://docs.rs/axum/0.6.4/axum/extract/struct.State.html#for-library-authors
                let state = AppState::from_ref(state);

                let auth_user: AuthUser = serde_json::from_str(&c)?;
                let user_id = auth_user.id;

                let pool = state.pool().await;
                let mut conn = pool.get().await?;
                let user = User::get(&mut conn, user_id).await?;

                let permission_group = PermissionGroup::try_from(user.permission_group)?;
                let permissions = user.permissions(&mut conn).await?;

                let extractor = UserExtractor(CurrentUser {
                    id: user.id,
                    is_admin: user.is_admin,
                    username: auth_user.username,
                    permission_group,
                    permissions
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
        let user_ext = UserExtractor::from_request_parts(parts, state).await?;
        let cu = user_ext.0;

        if !cu.is_admin {
            Err(AppError::AuthorizationError("only an admin can access this route.".to_string()))
        } else {
            Ok(Self)
        }
    }
}

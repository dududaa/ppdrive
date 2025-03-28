use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use reqwest::{header::AUTHORIZATION, StatusCode};
use serde::Deserialize;

use crate::{errors::AppError, models::user::User, state::AppState, utils::get_env};

#[derive(Deserialize)]
pub struct AuthUser {
    id: i32,
    username: String,
}

pub struct CurrentUser {
    id: i32,
    username: String,
    is_admin: bool,
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
                let User { id, is_admin, .. } = User::get(&mut conn, user_id).await?;

                let extractor = UserExtractor(CurrentUser {
                    id,
                    is_admin,
                    username: auth_user.username,
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

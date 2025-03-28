use axum::{
    extract::{Path, State},
    routing::{delete, post, get},
    Json, Router,
};
use axum_macros::debug_handler;
use serde::Deserialize;

use crate::{
    errors::AppError,
    models::{user::{User, UserSerializer}, Permission, PermissionGroup},
    state::AppState,
};

use crate::models::TryFromModel;

use super::extractors::{AdminRoute, UserExtractor};

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub permission_group: PermissionGroup,
    pub permissions: Option<Vec<Permission>>,
    // pub assign_root_folder: Option<bool>,
    // pub root_folder: Option<String>,
    // pub folder_max_size: Option<u64>
}

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    UserExtractor(_): UserExtractor,
    AdminRoute: AdminRoute,
    Json(data): Json<CreateUserRequest>,
) -> Result<Json<i32>, AppError> {
    let pool = state.pool().await;
    let mut conn = pool.get().await?;

    let user_id = User::create(&mut conn, data).await?;
    Ok(Json(user_id))
}

#[debug_handler]
async fn get_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    UserExtractor(_): UserExtractor,
    AdminRoute: AdminRoute,
) -> Result<Json<UserSerializer>, AppError> {
    let pool = state.pool().await;
    let mut conn = pool.get().await?;

    let user_id = id.parse::<i32>().map_err(|err| {
        AppError::InternalServerError(format!("unable to parse user id '{id}': {err}"))
    })?;

    let user = User::get(&mut conn, user_id).await?;
    let data = UserSerializer::try_from_model(&mut conn, user).await?;

    Ok(Json(data))
}

#[debug_handler]
async fn delete_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    UserExtractor(_): UserExtractor,
    AdminRoute: AdminRoute,
) -> Result<String, AppError> {
    let pool = state.pool().await;
    let mut conn = pool.get().await?;

    let user_id = id.parse::<i32>().map_err(|err| {
        AppError::InternalServerError(format!("unable to parse user id '{id}': {err}"))
    })?;
    User::delete(&mut conn, user_id).await?;

    Ok("operation successful".to_string())
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/user", post(create_user))
        .route("/user/:id", get(get_user))
        .route("/user/:id", delete(delete_user))
}

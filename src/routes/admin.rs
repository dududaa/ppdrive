use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use axum_macros::debug_handler;
use serde::Deserialize;

use crate::{
    errors::AppError,
    models::{
        permission::{Permission, PermissionGroup},
        user::{User, UserSerializer},
    },
    state::AppState,
};

use crate::models::IntoSerializer;

use super::extractors::ClientRoute;

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub permission_group: PermissionGroup,
    pub permissions: Option<Vec<Permission>>,
    pub root_folder: Option<String>,
    pub folder_max_size: Option<i64>,
}

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
    Json(data): Json<CreateUserRequest>,
) -> Result<String, AppError> {
    let user_id = User::create(&state, data).await?;

    Ok(user_id.to_string())
}

#[debug_handler]
async fn get_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
) -> Result<Json<UserSerializer>, AppError> {
    let user = User::get_by_pid(&state, &id).await?;
    let data = user.into_serializer(&state).await?;

    Ok(Json(data))
}

#[debug_handler]
async fn delete_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
) -> Result<String, AppError> {
    let user_id = id.parse::<i32>().map_err(|err| {
        AppError::InternalServerError(format!("unable to parse user id '{id}': {err}"))
    })?;
    let user = User::get(&state, &user_id).await?;
    user.delete(&state).await?;

    Ok("operation successful".to_string())
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/user", post(create_user))
        .route("/user/:id", get(get_user))
        .route("/user/:id", delete(delete_user))
}

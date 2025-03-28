use axum::{extract::State, routing::post, Json, Router};
use axum_macros::debug_handler;
use serde::Deserialize;

use crate::{errors::AppError, models::{Permission, PermissionGroup, user::User}, state::AppState};

use super::extractors::{AdminRoute, UserExtractor};

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub permission_group: PermissionGroup,
    pub permissions: Option<Vec<Permission>>
}

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    UserExtractor(_): UserExtractor,
    AdminRoute: AdminRoute,
    Json(data): Json<CreateUserRequest>
) -> Result<Json<i32>, AppError> {
    let pool = state.pool().await;
    let mut conn = pool.get().await?;

    let user_id = User::create(&mut conn, data).await?;
    Ok(Json(user_id))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/user", post(create_user))
}
use axum::{
    body::Body,
    extract::{Path, State},
    http::header::CONTENT_TYPE,
    response::Response,
};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};

use crate::{
    errors::AppError,
    models::{asset::Asset, user::UserRole},
    state::AppState,
};

use std::path::Path as StdPath;

pub mod client;
pub mod creator;
mod extractors;

#[derive(Deserialize)]
pub struct CreateUserRequest {
    /// folder name where user's assets are stored.
    /// If this is not specified, user's assets will be stored in application root folder.
    pub root_folder: Option<String>,

    /// max size set for user's `root` folder (in bytes).
    /// When this is exceeded, user will no longer be able to create an asset.
    pub folder_max_size: Option<i64>,

    /// User's role. Please see [UserRole](crate::models::user::UserRole).
    pub role: UserRole,
}

#[derive(Deserialize)]
pub struct LoginCredentials {
    pub id: String,
    pub password: Option<String>,
    pub exp: Option<i64>,
}

#[derive(Serialize)]
pub struct LoginToken {
    token: String,
    exp: i64,
}

#[debug_handler]
pub async fn get_asset(
    Path(asset_path): Path<String>,
    State(state): State<AppState>,
) -> Result<Response<Body>, AppError> {
    let asset = Asset::get_by_path(&state, &asset_path).await?;

    if asset.public {
        let path = StdPath::new(&asset.asset_path);

        if path.exists() {
            if path.is_file() {
                let content = tokio::fs::read(path).await?;
                let mime_type = mime_guess::from_path(path).first_or_octet_stream();
                let resp = Response::builder()
                    .header(CONTENT_TYPE, mime_type.to_string())
                    .body(Body::from(content))
                    .map_err(|err| AppError::InternalServerError(err.to_string()))?;

                Ok(resp)
            } else {
                Err(AppError::NotImplemented(
                    "folder view yet to be implemented".to_string(),
                ))
            }
        } else {
            Err(AppError::NotFound(format!(
                "asset '{asset_path}' not found"
            )))
        }
    } else {
        Err(AppError::InternalServerError("access denied".to_string()))
    }
}

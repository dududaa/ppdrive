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
    models::{
        asset::{Asset, AssetSharing, AssetType},
        user::UserRole,
    },
    state::AppState,
};

use std::path::Path as StdPath;

pub mod client;
mod extractors;
pub mod manager;

#[derive(Deserialize)]
pub struct CreateUserOptions {
    pub partition: Option<String>,
    pub partition_size: Option<i64>,
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

#[derive(Default, Deserialize)]
pub struct CreateAssetOptions {
    /// Destination path where asset should be created
    pub path: String,

    /// The type of asset - whether it's a file or folder
    pub asset_type: AssetType,

    /// Asset's visibility. Public assets can be read/accessed by everyone. Private assets can be
    /// viewed ONLY by permission.
    pub public: Option<bool>,

    /// Set a custom path for your asset instead of the one auto-generated from
    /// from `path`. This useful if you'd like to conceal your original asset path.
    /// Custom path must be available in that no other asset is already using it in the entire app.
    ///
    /// Your original asset path makes url look like this `https://mydrive.com/images/somewhere/my-image.png/`.
    /// Using custom path, you can conceal the original path: `https://mydrive.com/some/hidden-path`
    pub custom_path: Option<String>,

    /// If `asset_type` is [AssetType::Folder], we determine whether we should force-create it's parents folder if they
    /// don't exist. Asset creation will result in error if `create_parents` is `false` and folder parents don't exist.
    pub create_parents: Option<bool>,

    /// Users to share this asset with. This can only be set if `public` option is false
    pub sharing: Option<Vec<AssetSharing>>,
}

#[debug_handler]
pub async fn get_asset(
    Path((asset_type, asset_path)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Response<Body>, AppError> {
    let asset_type: AssetType = serde_json::from_str(&asset_type)?;
    let asset = Asset::get_by_path(&state, &asset_path, &asset_type).await?;

    // if asset has custom path and custom path is not provided in url,
    // we return an error. The purpose of custom path is to conceal the
    // original path
    if let Some(custom_path) = asset.custom_path() {
        if custom_path != &asset_path {
            return Err(AppError::NotFound("asset not found".to_string()));
        }
    }

    // find and serve asset
    if *asset.public() {
        let path = StdPath::new(asset.path());

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

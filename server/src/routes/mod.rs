use axum::{
    body::Body,
    extract::{Path, State},
    http::header::CONTENT_TYPE,
    response::Response,
};
use axum_macros::debug_handler;
use extractors::ExtractUser;
use serde::{Deserialize, Serialize};

use crate::{errors::AppError, state::AppState};

use ppdrive_core::{
    models::asset::{AssetType, Assets},
    tools::secrets::SECRETS_FILENAME,
};

use std::path::Path as StdPath;

pub mod client;
mod extractors;
pub mod protected;

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
    Path((asset_type, mut asset_path)): Path<(AssetType, String)>,
    State(state): State<AppState>,
    user_extractor: Option<ExtractUser>,
) -> Result<Response<Body>, AppError> {
    if asset_path.ends_with("/") {
        asset_path = asset_path.trim_end_matches("/").to_string();
    }

    if &asset_path == SECRETS_FILENAME {
        return Err(AppError::PermissionDenied("access denied".to_string()));
    }

    let db = state.db();
    let asset = Assets::get_by_path(db, &asset_path, &asset_type).await?;
    let current_user = user_extractor.map(|ext| ext.0);

    // if asset has custom path and custom path is not provided in url,
    // we return an error. The purpose of custom path is to conceal the
    // original path
    if let Some(custom_path) = asset.custom_path() {
        if custom_path != &asset_path {
            return Err(AppError::NotFound("asset not found".to_string()));
        }
    }

    // check if current user has read permission
    if !asset.public() {
        match &current_user {
            Some(current_user) => {
                let can_read = current_user.can_read_asset(&state, &asset.id()).await;

                if (current_user.id() != asset.user_id()) && can_read.is_err() {
                    return Err(AppError::PermissionDenied("permission denied".to_string()));
                }
            }
            None => {
                return Err(AppError::PermissionDenied("permission denied".to_string()));
            }
        }
    }

    let path = StdPath::new(asset.path());
    match asset_type {
        AssetType::File => {
            if path.exists() && path.is_file() {
                let content = tokio::fs::read(path).await?;
                let mime_type = mime_guess::from_path(path).first_or_octet_stream();
                let resp = Response::builder()
                    .header(CONTENT_TYPE, mime_type.to_string())
                    .body(Body::from(content))
                    .map_err(|err| AppError::InternalServerError(err.to_string()))?;

                Ok(resp)
            } else {
                Err(AppError::NotFound(format!(
                    "asset record found but path '{asset_path}' does not exist if filesystem for '{asset_type}'."
                )))
            }
        }
        AssetType::Folder => {
            if path.exists() && path.is_dir() {
                let mut contents = tokio::fs::read_dir(path).await?;
                let mut filenames = Vec::new();

                // let's attempt to read folder contents, checking for
                // asset ownership all along
                while let Ok(Some(entry)) = contents.next_entry().await {
                    let path = entry.path();
                    let filename = entry.file_name();

                    if let (Some(path_str), Some(filename)) = (path.to_str(), filename.to_str()) {
                        let asset_type = if path.is_file() {
                            AssetType::File
                        } else {
                            AssetType::Folder
                        };

                        let asset = Assets::get_by_path(db, path_str, &asset_type).await;
                        if let Ok(asset) = asset {
                            let html =
                                format!("<li><a href='/{}'>{filename}</a></li>", asset.url_path());

                            if *asset.public() {
                                filenames.push(html);
                            } else {
                                if let Some(auth) = &current_user {
                                    let can_read = auth.can_read_asset(&state, &asset.id()).await;
                                    if (auth.id() == asset.user_id()) || can_read.is_ok() {
                                        filenames.push(html);
                                    }
                                }
                            }
                        }
                    }
                }

                let content = if filenames.is_empty() {
                    "<p>No content found.</p>".to_string()
                } else {
                    format!(r#"<ul>{}</ul>"#, filenames.join("\n"))
                };

                let body = format!(
                    r#"
                    <DOCTYPE! html>
                    <html>
                        {content}
                    </html>
                "#
                );
                let resp = Response::builder()
                    .header(CONTENT_TYPE, "text/html")
                    .body(Body::from(body))
                    .map_err(|err| AppError::InternalServerError(err.to_string()))?;

                Ok(resp)
            } else {
                Err(AppError::NotFound(format!(
                    "asset record found but path '{asset_path}' does not exist if filesystem for '{asset_type}'."
                )))
            }
        }
    }
}

use std::path::Path;

use axum::{
    body::Body,
    extract::{Multipart, Path as ReqPath, State},
    http::Response,
    routing::{get, post},
    Router,
};
use axum_macros::debug_handler;
use reqwest::header;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use crate::{
    errors::AppError,
    models::asset::{Asset, CreateAssetOptions},
    state::AppState,
};

use super::extractors::UserExtractor;

#[debug_handler]
async fn get_asset(
    ReqPath(asset_path): ReqPath<String>,
    State(state): State<AppState>,
) -> Result<Response<Body>, AppError> {
    let pool = state.pool().await;
    let mut conn = pool.get().await?;

    let asset = Asset::get_by_path(&mut conn, asset_path.clone()).await?;

    if asset.public {
        let path = Path::new(&asset.asset_path);
        
        if path.exists() {
            if path.is_file() {
                let content = tokio::fs::read(path).await?;
                let mime_type = mime_guess::from_path(path).first_or_octet_stream();
                let resp = Response::builder()
                    .header(header::CONTENT_TYPE, mime_type.to_string())
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

#[debug_handler]
async fn create_asset(
    State(state): State<AppState>,
    UserExtractor(current_user): UserExtractor,
    mut multipart: Multipart,
) -> Result<String, AppError> {
    if current_user.can_create() {
        let user_id = current_user.id;

        let mut opts = CreateAssetOptions::default();
        let mut tmp_file = None;

        while let Some(mut field) = multipart.next_field().await? {
            let name = field.name().unwrap_or("").to_string();

            if name == "options" {
                let data = field.text().await?;
                opts = serde_json::from_str(&data)?;
            } else if name == "file" {
                let tmp_name = Uuid::new_v4().to_string();
                let tmp_path = format!("./tmp/{tmp_name}");
                let mut file = File::create(&tmp_path).await?;

                while let Some(chunk) = field.chunk().await? {
                    file.write_all(&chunk).await?;
                }

                tmp_file = Some(tmp_path);
            }
        }

        let pool = state.pool().await;
        let mut conn = pool.get().await?;

        let path = Asset::create_or_update(&mut conn, &user_id, opts, tmp_file).await?;
        Ok(path)
    } else {
        Err(AppError::AuthorizationError(
            "permission denied".to_string(),
        ))
    }
}

pub fn asset_routes() -> Router<AppState> {
    Router::new()
        .route("/*asset_path", get(get_asset))
        .route("/create", post(create_asset))
}

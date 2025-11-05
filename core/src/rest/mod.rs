use axum::{
    body::Body,
    extract::{Multipart, Path, State, multipart::MultipartError},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};

use axum_macros::debug_handler;
use ppd_bk::models::asset::AssetType;
use ppd_fs::{
    AssetBody,
    auth::{create_or_update_asset, delete_asset},
    opts::CreateAssetOptions,
    read_asset,
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    HandlerResult,
    errors::HandlerError,
    prelude::state::HandlerState,
    rest::extractors::{BucketSizeValidator, UserExtractor},
};

pub mod extractors;

#[debug_handler]
pub async fn get_asset(
    Path((asset_type, asset_path)): Path<(AssetType, String)>,
    State(state): State<HandlerState>,
    user: Option<UserExtractor>,
) -> Result<Response<Body>, HandlerError> {
    let db = state.db();
    let user_id = user.map(|u| *u.id());
    let body = read_asset(db, &asset_path, &asset_type, &user_id).await?;

    let body = match body {
        AssetBody::File(mime, content) => Response::builder()
            .header(CONTENT_TYPE, mime.to_string())
            .body(Body::from(content)),
        AssetBody::Folder(content) => Response::builder()
            .header(CONTENT_TYPE, "text/html")
            .body(Body::from(content)),
    };

    let resp = body.map_err(|err| HandlerError::InternalError(err.to_string()))?;
    Ok(resp)
}

pub async fn create_asset_user(
    user_id: &u64,
    mut multipart: Multipart,
    state: HandlerState,
) -> HandlerResult<String> {
    let mut opts = CreateAssetOptions::default();
    let mut tmp_file = None;
    let mut filesize = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field
            .name()
            .ok_or(HandlerError::PermissionError(
                "empty fields are not allowed".to_string(),
            ))?
            .to_string();

        if name == "options" {
            let data = field.text().await?;

            opts = serde_json::from_str(&data)
                .map_err(|err| HandlerError::InternalError(err.to_string()))?;
        } else if name == "file" {
            let tmp_name = Uuid::new_v4().to_string();
            let mut tmp_path = std::env::temp_dir();
            tmp_path.push(tmp_name);

            let mut file = tokio::fs::File::create(&tmp_path).await?;

            let data = field.bytes().await?;
            file.write_all(&data).await?;

            filesize = Some(file.metadata().await?.len());
            tmp_file = Some(tmp_path);
        }
    }

    let db = state.db();
    let path = create_or_update_asset(db, user_id, &opts, &tmp_file, &filesize).await?;

    Ok(path)
}

pub async fn delete_asset_user(
    user_id: &u64,
    path: &str,
    asset_type: &AssetType,
    state: HandlerState,
) -> HandlerResult<()> {
    let db = state.db();
    delete_asset(db, user_id, path, asset_type).await?;

    Ok(())
}

impl From<MultipartError> for HandlerError {
    fn from(value: MultipartError) -> Self {
        HandlerError::InternalError(value.to_string())
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            HandlerError::AuthorizationError(msg) => (StatusCode::UNAUTHORIZED, msg),
            HandlerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            HandlerError::PermissionError(msg) => (StatusCode::FORBIDDEN, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        resp.into_response()
    }
}

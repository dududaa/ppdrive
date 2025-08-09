use axum::{
    body::Body,
    extract::{Path, State},
    http::header::CONTENT_TYPE,
    response::Response,
};
use axum_macros::debug_handler;
use ppd_bk::models::asset::AssetType;
use ppd_fs::{read_asset, AssetBody};
use ppd_shared::tools::SECRETS_FILENAME;

use crate::{errors::ServerError, extractors::ClientUser, state::AppState};

#[debug_handler]
pub async fn get_asset(
    Path((asset_type, mut asset_path)): Path<(AssetType, String)>,
    State(state): State<AppState>,
    user: Option<ClientUser>,
) -> Result<Response<Body>, ServerError> {
    if asset_path.ends_with("/") {
        asset_path = asset_path.trim_end_matches("/").to_string();
    }

    if &asset_path == SECRETS_FILENAME {
        return Err(ServerError::PermissionDenied("access denied".to_string()));
    }

    let db = state.db();
    let user_id = user.map(|u| u.0.id().clone());
    let body = read_asset(db, &asset_path, &asset_type, &user_id).await?;

    let body = match body {
        AssetBody::File(mime, content) => Response::builder()
            .header(CONTENT_TYPE, mime.to_string())
            .body(Body::from(content)),
        AssetBody::Folder(content) => Response::builder()
            .header(CONTENT_TYPE, "text/html")
            .body(Body::from(content)),
    };

    let resp = body.map_err(|err| ServerError::InternalError(err.to_string()))?;
    Ok(resp)
}

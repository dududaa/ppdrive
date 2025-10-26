use axum::{
    Json,
    extract::{Multipart, Path, State},
};
use axum_macros::debug_handler;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use crate::errors::ServerError;
use ppdrive::{prelude::state::HandlerState, rest::extractors::ClientUserExtractor};
use ppd_bk::models::{
    IntoSerializer,
    asset::{AssetType, Assets},
    bucket::{Buckets, CreateBucketOptions},
    user::{UserSerializer, Users},
};
use ppd_shared::tools::SECRETS_FILENAME;

use ppd_fs::{auth::create_or_update_asset, opts::CreateAssetOptions};

#[debug_handler]
pub async fn get_user(
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
) -> Result<Json<UserSerializer>, ServerError> {
    let db = state.db();
    let user_model = Users::get(db, user.id()).await?;
    let data = user_model.into_serializer(db).await?;

    Ok(Json(data))
}

#[debug_handler]
pub async fn create_user_bucket(
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, ServerError> {
    let db = state.db();
    let id = Buckets::create_by_user(db, data, *user.id()).await?;

    Ok(id)
}

#[debug_handler]
pub async fn create_asset(
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
    mut multipart: Multipart,
) -> Result<String, ServerError> {
    let mut opts = CreateAssetOptions::default();
    let mut tmp_file = None;
    let mut filesize = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();

        if name == "options" {
            let data = field.text().await?;
            opts = serde_json::from_str(&data)?;
        } else if name == "file" {
            let tmp_name = Uuid::new_v4().to_string();
            let mut tmp_path = std::env::temp_dir();
            tmp_path.push(tmp_name);

            let mut file = File::create(&tmp_path).await?;

            let data = field.bytes().await?;
            file.write_all(&data).await?;

            filesize = Some(file.metadata().await?.len());
            tmp_file = Some(tmp_path);
        }
    }

    // options validations
    if opts.asset_path.is_empty() {
        return Err(ServerError::InternalError(
            "asset_path field is required".to_string(),
        ));
    }

    if opts.asset_path == SECRETS_FILENAME {
        return Err(ServerError::AuthorizationError(
            "asset_path '{SECRET_FILE}' is reserved. please choose another path.".to_string(),
        ));
    }

    let db = state.db();
    create_or_update_asset(db, user.id(), &opts, &tmp_file, &filesize).await?;
    Ok("operation successful!".to_string())
}

#[debug_handler]
pub async fn delete_asset(
    Path((asset_type, asset_path)): Path<(AssetType, String)>,
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
) -> Result<String, ServerError> {
    let db = state.db();
    let asset = Assets::get_by_path(db, &asset_path, &asset_type).await?;

    if asset.user_id() == user.id() {
        asset.delete(db).await?;
        Ok("operation successful".to_string())
    } else {
        Err(ServerError::AuthorizationError(
            "permission denied".to_string(),
        ))
    }
}

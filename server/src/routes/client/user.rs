use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use axum_macros::debug_handler;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use crate::{errors::AppError, state::AppState, utils::mb_to_bytes};

use ppdrive_core::{
    models::{
        asset::{AssetType, Assets},
        bucket::Buckets,
        user::{UserSerializer, Users},
        IntoSerializer,
    },
    options::CreateAssetOptions,
    tools::secrets::SECRETS_FILENAME,
};

use crate::routes::extractors::ClientUser;

#[debug_handler]
pub async fn get_user(
    State(state): State<AppState>,
    ClientUser(user): ClientUser,
) -> Result<Json<UserSerializer>, AppError> {
    let db = state.db();
    let user_model = Users::get(db, user.id()).await?;
    let data = user_model.into_serializer(db).await?;

    Ok(Json(data))
}

#[debug_handler]
pub async fn create_asset(
    State(state): State<AppState>,
    ClientUser(user): ClientUser,
    mut multipart: Multipart,
) -> Result<String, AppError> {
    let user_id = user.id();

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

    // validations
    if opts.asset_path.is_empty() {
        return Err(AppError::InternalServerError(
            "asset_path field is required".to_string(),
        ));
    }

    if &opts.asset_path == SECRETS_FILENAME {
        return Err(AppError::AuthorizationError(
            "asset_path '{SECRET_FILE}' is reserved. please choose another name.".to_string(),
        ));
    }

    let db = state.db();
    let bucket = Buckets::get_by_pid(db, user_id, &opts.bucket).await?;

    // extract destination path
    let asset_path = &opts.asset_path;
    let partition = bucket.partition().as_deref();
    let dest = partition.map_or(asset_path.to_string(), |rf| format!("{rf}/{asset_path}"));
    let dest = std::path::Path::new(&dest);

    if let (Some(filesize), Some(max_size)) = (filesize, bucket.partition_size()) {
        let cfz = bucket.content_size().await?;
        let total_size = cfz + filesize;
        if total_size > mb_to_bytes(*max_size as usize) as u64 {
            if let Some(tmp_file) = tmp_file {
                tokio::fs::remove_file(tmp_file).await?;
            }

            return Err(AppError::InternalServerError(
                "the total partition size assigned to this user is exceeded.".to_string(),
            ));
        }
    }

    // create asset record
    let path = Assets::create_or_update(db, user_id, &bucket.id(), opts, &tmp_file, dest).await?;
    if let Some(tmp_file) = &tmp_file {
        if let Err(err) = tokio::fs::remove_file(tmp_file).await {
            tracing::error!("unable to remove {tmp_file:?}: {err}")
        }
    }

    Ok(path)
}

#[debug_handler]
pub async fn delete_asset(
    Path((asset_type, asset_path)): Path<(AssetType, String)>,
    State(state): State<AppState>,
    ClientUser(user): ClientUser,
) -> Result<String, AppError> {
    let db = state.db();
    let asset = Assets::get_by_path(db, &asset_path, &asset_type).await?;

    if asset.user_id() == user.id() {
        asset.delete(db).await?;
        Ok("operation successful".to_string())
    } else {
        Err(AppError::AuthorizationError(
            "permission denied".to_string(),
        ))
    }
}

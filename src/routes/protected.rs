use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use axum_macros::debug_handler;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use crate::{
    errors::AppError,
    models::{
        asset::{Asset, AssetType},
        user::{User, UserSerializer},
        IntoSerializer,
    },
    state::AppState,
    utils::{get_env, mb_to_bytes, tools::secrets::SECRET_FILE},
};

use super::{
    extractors::{ExtractUser, ManagerRoute},
    CreateAssetOptions,
};

#[debug_handler]
async fn get_user(
    State(state): State<AppState>,
    ExtractUser(user): ExtractUser,
    ManagerRoute: ManagerRoute,
) -> Result<Json<UserSerializer>, AppError> {
    let user_model = User::get(&state, user.id()).await?;
    let data = user_model.into_serializer(&state).await?;

    Ok(Json(data))
}

#[debug_handler]
async fn create_asset(
    State(state): State<AppState>,
    ExtractUser(user): ExtractUser,
    ManagerRoute: ManagerRoute,
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

    let cfz = user.current_folder_size().await?;
    if let (Some(ufz), Some(filesize), Some(max_size)) = (cfz, filesize, user.folder_max_size()) {
        let total_size = ufz + filesize;
        if total_size > mb_to_bytes(*max_size as usize) as u64 {
            if let Some(tmp_file) = tmp_file {
                tokio::fs::remove_file(tmp_file).await?;
            }

            return Err(AppError::InternalServerError(
                "the total partition size assigned to this user is exceeded.".to_string(),
            ));
        }
    }

    if opts.asset_path.is_empty() {
        return Err(AppError::InternalServerError(
            "asset_path field is required".to_string(),
        ));
    }

    if &opts.asset_path == SECRET_FILE {
        return Err(AppError::AuthorizationError(
            "asset_path '{SECRET_FILE}' is reserved. please choose another name.".to_string(),
        ));
    }

    let path = Asset::create_or_update(&state, user_id, opts, &tmp_file).await?;
    if let Some(tmp_file) = &tmp_file {
        if let Err(err) = tokio::fs::remove_file(tmp_file).await {
            tracing::info!("unable to remove {tmp_file:?}: {err}")
        }
    }

    Ok(path)
}

#[debug_handler]
async fn delete_asset(
    Path((asset_type, asset_path)): Path<(AssetType, String)>,
    State(state): State<AppState>,
    ExtractUser(user): ExtractUser,
    ManagerRoute: ManagerRoute,
) -> Result<String, AppError> {
    let asset = Asset::get_by_path(&state, &asset_path, &asset_type).await?;

    if asset.user_id() == user.id() {
        asset.delete(&state).await?;
        Ok("operation successful".to_string())
    } else {
        Err(AppError::AuthorizationError(
            "permission denied".to_string(),
        ))
    }
}

/// Routes accessible to creators
pub fn protected_routes() -> Result<Router<AppState>, AppError> {
    let max_upload_size = get_env("PPDRIVE_MAX_UPLOAD_SIZE")?;
    let max = max_upload_size
        .parse::<usize>()
        .map_err(|err| AppError::InitError(err.to_string()))?;

    let limit = mb_to_bytes(max);

    let router = Router::new()
        .route("/user", get(get_user))
        .route("/asset", post(create_asset))
        .layer(DefaultBodyLimit::max(limit))
        .route("/asset/:asset_type/:asset_path", delete(delete_asset));

    Ok(router)
}

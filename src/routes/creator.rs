use axum::{
    extract::{Multipart, Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use axum_macros::debug_handler;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use crate::{
    errors::AppError,
    models::{
        asset::{Asset, CreateAssetOptions},
        user::{User, UserSerializer},
        IntoSerializer,
    },
    state::AppState,
};

use super::extractors::ExtractUser;

#[debug_handler]
async fn get_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<UserSerializer>, AppError> {
    let user = User::get_by_pid(&state, &id).await?;
    let data = user.into_serializer(&state).await?;

    Ok(Json(data))
}

#[debug_handler]
async fn create_asset(
    State(state): State<AppState>,
    ExtractUser(user): ExtractUser,
    mut multipart: Multipart,
) -> Result<String, AppError> {
    if user.can_create() {
        let user_id = user.id();

        let mut opts = CreateAssetOptions::default();
        let mut tmp_file = None;
        let mut filesize = None;

        while let Some(mut field) = multipart.next_field().await? {
            let name = field.name().unwrap_or("").to_string();

            if name == "options" {
                let data = field.text().await?;
                opts = serde_json::from_str(&data)?;
            } else if name == "file" {
                let tmp_name = Uuid::new_v4().to_string();
                let mut tmp_path = std::env::temp_dir();
                tmp_path.push(tmp_name);

                let mut file = File::create(&tmp_path).await?;
                while let Some(chunk) = field.chunk().await? {
                    file.write_all(&chunk).await?;
                }

                filesize = Some(file.metadata().await?.len());
                tmp_file = Some(tmp_path);
            }
        }

        let cfz = user.current_folder_size().await?;
        if let (Some(ufz), Some(filesize), Some(max_size)) = (cfz, filesize, user.folder_max_size())
        {
            let total_size = ufz + filesize;
            if total_size > (*max_size as u64) {
                return Err(AppError::InternalServerError(
                    "the total partition size assigned to this user is exceeded.".to_string(),
                ));
            }
        }

        let path = Asset::create_or_update(&state, user_id, opts, tmp_file).await?;
        Ok(path)
    } else {
        Err(AppError::AuthorizationError(
            "permission denied".to_string(),
        ))
    }
}

#[debug_handler]
async fn delete_asset(
    Path(asset_path): Path<String>,
    State(state): State<AppState>,
) -> Result<String, AppError> {
    Asset::delete(&state, &asset_path).await?;

    Ok("operation successful".to_string())
}

/// Routes accessible to creators
pub fn creator_routes() -> Router<AppState> {
    Router::new()
        .route("/user", get(get_user))
        .route("/asset", post(create_asset))
        .route("/asset", delete(delete_asset))
}

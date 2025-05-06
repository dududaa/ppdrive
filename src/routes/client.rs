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

use super::{
    extractors::{ClientRoute, ClientUser},
    CreateUserRequest,
};

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
    Json(data): Json<CreateUserRequest>,
) -> Result<String, AppError> {
    let user_id = User::create(&state, data).await?;

    Ok(user_id.to_string())
}

#[debug_handler]
async fn get_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
) -> Result<Json<UserSerializer>, AppError> {
    let user = User::get_by_pid(&state, &id).await?;
    let data = user.into_serializer(&state).await?;

    Ok(Json(data))
}

#[debug_handler]
async fn delete_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
) -> Result<String, AppError> {
    let user_id = id.parse::<i32>().map_err(|err| {
        AppError::InternalServerError(format!("unable to parse user id '{id}': {err}"))
    })?;
    let user = User::get(&state, &user_id).await?;
    user.delete(&state).await?;

    Ok("operation successful".to_string())
}

#[debug_handler]
async fn create_asset(
    State(state): State<AppState>,
    ClientRoute: ClientRoute,
    ClientUser(user): ClientUser,
    mut multipart: Multipart,
) -> Result<String, AppError> {
    if user.can_create() {
        let user_id = user.id;

        let mut opts = CreateAssetOptions::default();
        let mut tmp_file = None;

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

                tmp_file = Some(tmp_path);
            }
        }

        let path = Asset::create_or_update(&state, &user_id, opts, tmp_file).await?;
        Ok(path)
    } else {
        Err(AppError::AuthorizationError(
            "permission denied".to_string(),
        ))
    }
}

/// Routes to be requested by PPDRIVE [Client].
pub fn client_routes() -> Router<AppState> {
    Router::new()
        .route("/user", post(create_user))
        .route("/user/:id", get(get_user))
        .route("user/:id", delete(delete_user))
}

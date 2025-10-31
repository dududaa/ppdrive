use std::sync::Arc;

use tokio::{fs::File, io::AsyncWriteExt};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    routing::{delete, get, post},
};
use axum_macros::debug_handler;
use ppd_fs::{auth::create_or_update_asset, opts::CreateAssetOptions};
use uuid::Uuid;

use crate::errors::ServerError;

use ppd_shared::{
    api::{CreateBucketOptions, LoginTokens, UserCredentials}, opts::ServiceConfig, tools::{SECRETS_FILENAME, mb_to_bytes}
};
use ppdrive::{
    jwt::LoginOpts,
    prelude::state::HandlerState,
    rest::extractors::{BucketSizeValidator, UserExtractor},
    tools::{check_password, make_password},
};

use ppd_bk::models::{
    IntoSerializer,
    asset::{AssetType, Assets},
    bucket::Buckets,
    user::{UserSerializer, Users},
};

mod errors;

#[debug_handler]
async fn register_user(
    State(state): State<HandlerState>,
    Json(data): Json<UserCredentials>,
) -> Result<String, ServerError> {
    let db = state.db();
    let UserCredentials { username, password } = data;

    let password = make_password(&password);
    let user_id = Users::create(db, username, password).await?;

    Ok(user_id)
}

#[debug_handler]
async fn login_user(
    State(state): State<HandlerState>,
    Json(data): Json<UserCredentials>,
) -> Result<Json<LoginTokens>, ServerError> {
    let db = state.db();
    let config = state.config();
    let secrets = state.secrets();

    let UserCredentials { username, password } = data;
    let user = Users::get_by_key(db, "username", &username)
        .await
        .map_err(|err| ServerError::InternalError(err.to_string()))?
        .ok_or(ServerError::AuthorizationError(format!(
            "user with username '{username}' does not exist"
        )))?;

    let hashed = user.password().clone().unwrap_or(String::new());
    check_password(&password, &hashed)?;

    let login = LoginOpts {
        user_id: &user.id(),
        config: &config,
        jwt_secret: secrets.jwt_secret(),
        access_exp: None,
        refresh_exp: None,
        user_max_bucket: *user.max_bucket_size()
    };

    let tokens = login.tokens()?;
    Ok(Json(tokens))
}

#[debug_handler]
pub async fn get_user(
    State(state): State<HandlerState>,
    user: UserExtractor,
) -> Result<Json<UserSerializer>, ServerError> {
    let db = state.db();
    let user_model = Users::get(db, user.id()).await?;
    let data = user_model.into_serializer(db).await?;

    Ok(Json(data))
}

#[debug_handler]
pub async fn create_user_bucket(
    State(state): State<HandlerState>,
    user: UserExtractor,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, ServerError> {
    let db = state.db();

    user.validate_bucket_size(db, &data.partition_size).await?;
    let id = Buckets::create_by_user(db, data, *user.id()).await?;

    Ok(id)
}

#[debug_handler]
pub async fn create_asset(
    State(state): State<HandlerState>,
    user: UserExtractor,
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
    user: UserExtractor,
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

/// Routes for external clients.
fn routes(config: Arc<ServiceConfig>) -> Router<HandlerState> {
    let limit = mb_to_bytes(config.base.max_upload_size);

    Router::new()
        .route("/user", get(get_user))
        .route("/user/register", post(register_user))
        .route("/user/login", post(login_user))
        .route("/user/asset", post(create_asset))
        .layer(DefaultBodyLimit::max(limit))
        .route("/user/asset/:asset_type/*asset_path", delete(delete_asset))
        .route("/user/bucket", post(create_user_bucket))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rest_direct(config: *const ServiceConfig) -> *mut Router<HandlerState> {
    let config = unsafe { Arc::from_raw(config) };
    let bx = Box::new(routes(config));
    Box::into_raw(bx)
}

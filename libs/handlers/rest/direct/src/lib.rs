use std::sync::Arc;

use tokio::runtime::Runtime;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    routing::{delete, get, post},
};
use axum_macros::debug_handler;

use crate::errors::ServerError;

use ppd_shared::{
    opts::{
        api::{CreateBucketOptions, LoginTokens, UserCredentials},
        internal::ServiceConfig,
    },
    tools::mb_to_bytes,
};
use ppdrive::{
    jwt::LoginOpts,
    prelude::state::HandlerState,
    rest::{
        create_asset_user, delete_asset_user,
        extractors::{BucketSizeValidator, UserExtractor},
    },
    tools::{check_password, make_password},
};

use ppd_bk::models::{
    IntoSerializer,
    asset::AssetType,
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
    let user_id = Users::create_direct(db, username, password).await?;

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
        user_max_bucket: *user.max_bucket_size(),
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

    user.validate_bucket_size(db, &data.size).await?;
    let id = Buckets::create_by_user(db, data, *user.id()).await?;

    Ok(id)
}

#[debug_handler]
pub async fn create_asset(
    State(state): State<HandlerState>,
    user: UserExtractor,
    multipart: Multipart,
) -> Result<String, ServerError> {
    let path = create_asset_user(user.id(), multipart, state).await?;
    Ok(path)
}

#[debug_handler]
pub async fn delete_asset(
    Path((asset_type, asset_path)): Path<(AssetType, String)>,
    State(state): State<HandlerState>,
    user: UserExtractor,
) -> Result<String, ServerError> {
    delete_asset_user(user.id(), &asset_path, &asset_type, state).await?;
    Ok("operation successful".to_string())
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
    let mut ptr = std::ptr::null_mut();

    if let Ok(rt) = Runtime::new() {
        let router = rt.block_on(async move { Box::new(routes(config)) });

        ptr = Box::into_raw(router);
    }

    ptr
}

#[cfg(feature = "test")]
/// test routers are designed to be loaded directly without tokio runtime. They're to be used
/// in test cases in order to prevent tokio runtime being initalized multiple times.
pub extern "C" fn test_router(config: *const ServiceConfig) -> *mut Router<HandlerState> {
    let config = unsafe { Arc::from_raw(config) };
    let bx = Box::new(routes(config));

    Box::into_raw(bx)
}

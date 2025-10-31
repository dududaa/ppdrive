use std::sync::Arc;

use auth::*;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    routing::{delete, get, post},
};
use axum_macros::debug_handler;

use crate::errors::ServerError;

use ppd_shared::{
    api::{CreateBucketOptions, CreateClientUser, LoginTokens, LoginUserClient},
    opts::ServiceConfig,
    tools::{SECRETS_FILENAME, mb_to_bytes},
};
use ppdrive::{
    jwt::LoginOpts,
    prelude::state::HandlerState,
    rest::extractors::{BucketSizeValidator, ClientExtractor},
};

use ppd_bk::models::{
    bucket::Buckets,
    user::{UserRole, Users},
};

mod auth;
mod errors;

#[debug_handler]
async fn create_user(
    State(state): State<HandlerState>,
    client: ClientExtractor,
    Json(data): Json<CreateClientUser>,
) -> Result<String, ServerError> {
    let db = state.db();
    let user_id = Users::create_by_client(db, *client.id(), data.max_bucket).await?;

    Ok(user_id)
}

#[debug_handler]
async fn login_user(
    State(state): State<HandlerState>,
    _: ClientExtractor,
    Json(data): Json<LoginUserClient>,
) -> Result<Json<LoginTokens>, ServerError> {
    let LoginUserClient {
        id,
        access_exp,
        refresh_exp,
    } = data;

    let db = state.db();
    let config = state.config();
    let secrets = state.secrets();

    let user = Users::get_by_pid(db, &id).await?;
    let login = LoginOpts {
        user_id: &user.id(),
        config: &config,
        jwt_secret: secrets.jwt_secret(),
        access_exp,
        refresh_exp,
    };

    let tokens = login.tokens()?;
    Ok(Json(tokens))
}

#[debug_handler]
async fn delete_user(
    Path(id): Path<String>,
    client: ClientExtractor,
    State(state): State<HandlerState>,
) -> Result<String, ServerError> {
    let db = state.db();
    let user = Users::get_by_pid(db, &id).await?;

    if let Some(client_id) = user.client_id() {
        println!("client {}, user-client {}", client.id(), client_id);
        if client_id != client.id() {
            return Err(ServerError::PermissionDenied(
                "client cannot delete this user".to_string(),
            ));
        }
    }

    match user.role()? {
        UserRole::Admin => Err(ServerError::AuthorizationError(
            "client cannot delete admin".to_string(),
        )),
        _ => {
            user.delete(db).await?;
            Ok("operation successful".to_string())
        }
    }
}

#[debug_handler]
async fn create_bucket(
    State(state): State<HandlerState>,
    client: ClientExtractor,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, ServerError> {
    let db = state.db();

    client
        .validate_bucket_size(db, &data.partition_size)
        .await?;
    if let Some(partition) = &data.partition
        && partition == SECRETS_FILENAME
    {
        return Err(ServerError::PermissionDenied(
            "partition name {SECRET_FILE} is not allowed".to_string(),
        ));
    }

    let bucket_id = Buckets::create_by_client(db, data, *client.id()).await?;
    Ok(bucket_id.to_string())
}

/// Routes for external clients.
fn routes(config: Arc<ServiceConfig>) -> Router<HandlerState> {
    let limit = mb_to_bytes(config.base.max_upload_size);

    Router::new()
        // Routes used by client for administrative tasks. Requests to these routes
        // require ppd-client-token header.
        .route("/user/login", post(login_user))
        .route("/user/register", post(create_user))
        .route("/user/:id", delete(delete_user))
        .route("/bucket", post(create_bucket))
        // Routes used by client to operate on behalf of a user. Access to these routes requires
        // both  `ppd-client-token` and `ppd-client-user` headers
        .route("/user", get(get_user))
        .route("/user/asset", post(create_asset))
        .layer(DefaultBodyLimit::max(limit))
        .route("/user/asset/:asset_type/*asset_path", delete(delete_asset))
        .route("/user/bucket", post(create_user_bucket))
}

#[unsafe(no_mangle)]
pub extern "C" fn rest_client(config: *const ServiceConfig) -> *mut Router<HandlerState> {
    let config = unsafe { Arc::from_raw(config) };
    let bx = Box::new(routes(config));
    Box::into_raw(bx)
}

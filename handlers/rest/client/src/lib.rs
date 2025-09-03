use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    routing::{delete, get, post},
};
use axum_macros::debug_handler;
use user::*;

use crate::errors::ServerError;
use handlers::prelude::{
    extractors::ClientRoute,
    jwt::{TokenType, create_jwt},
    opts::{CreateUserClient, LoginToken, LoginUserClient},
    state::HandlerState,
};
use ppd_shared::tools::{SECRETS_FILENAME, mb_to_bytes};

use ppd_bk::models::{
    bucket::{Buckets, CreateBucketOptions},
    user::{UserRole, Users},
};

mod errors;
mod user;

#[debug_handler]
async fn create_user(
    State(state): State<HandlerState>,
    client: ClientRoute,
    Json(data): Json<CreateUserClient>,
) -> Result<String, ServerError> {
    let db = state.db();
    let user_id = Users::create_by_client(db, *client.id(), data.max_bucket).await?;

    Ok(user_id.to_string())
}

#[debug_handler]
async fn login_user(
    State(state): State<HandlerState>,
    _: ClientRoute,
    Json(data): Json<LoginUserClient>,
) -> Result<Json<LoginToken>, ServerError> {
    let LoginUserClient {
        id,
        access_exp,
        refresh_exp,
    } = data;

    let db = state.db();
    let config = state.config();
    let secrets = state.secrets();

    let user = Users::get_by_pid(db, &id).await?;
    let default_access = config.auth.access_exp;
    let default_refresh = config.auth.refresh_exp;

    let access_exp = access_exp.unwrap_or(default_access);
    let refresh_exp = refresh_exp.unwrap_or(default_refresh);

    let access = if access_exp > 0 {
        let access_token = create_jwt(
            &user.id(),
            secrets.jwt_secret(),
            access_exp,
            TokenType::Access,
        )?;

        Some((access_token,access_exp))
    } else {
        None
    };

    let refresh = if refresh_exp > 0 {
        let refresh_token = create_jwt(
            &user.id(),
            secrets.jwt_secret(),
            access_exp,
            TokenType::Refresh,
        )?;

        Some((refresh_token, refresh_exp))
    } else {
        None
    };

    let data = LoginToken { access, refresh };

    Ok(Json(data))
}

#[debug_handler]
async fn delete_user(
    Path(id): Path<String>,
    client: ClientRoute,
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
    client: ClientRoute,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, ServerError> {
    let db = state.db();
    if let Some(partition) = &data.partition {
        if partition == SECRETS_FILENAME {
            return Err(ServerError::PermissionDenied(
                "partition name {SECRET_FILE} is not allowed".to_string(),
            ));
        }
    }

    let bucket_id = Buckets::create_by_client(db, data, *client.id()).await?;
    Ok(bucket_id.to_string())
}

/// Routes for external clients.
pub fn client_routes(max_upload_size: usize) -> Router<HandlerState> {
    let limit = mb_to_bytes(max_upload_size);

    Router::new()
        // Routes used by client for administrative tasks. Requests to these routes
        // require x-ppd-client header.
        .route("/user/register", post(create_user))
        .route("/user/login", post(login_user))
        .route("/user/:id", delete(delete_user))
        .route("/bucket", post(create_bucket))
        // Routes accessible to users created/managed by clients. Requests to these routes
        // do not required x-ppd-client header but may require authorization header
        // if config.auth.url is not provided.
        //
        // Client users may access these routes directly using authorization header (without)
        // contacting client server first.
        .route("/user", get(get_user))
        .route("/user/asset", post(create_asset))
        .layer(DefaultBodyLimit::max(limit))
        .route("/user/asset/:asset_type/*asset_path", delete(delete_asset))
        .route("/user/bucket", post(create_user_bucket))
}

#[unsafe(no_mangle)]
pub extern "C" fn load_router(max_upload_size: usize) -> *mut Router<HandlerState> {
    println!("loading router with max_upload size...");
    let router = client_routes(max_upload_size);
    let boxed = Box::new(router);

    println!("router is boxed...");

    Box::into_raw(boxed)
}

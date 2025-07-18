use axum::{
    extract::{DefaultBodyLimit, Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use axum_macros::debug_handler;
use user::*;

use crate::{
    errors::AppError,
    state::AppState,
    utils::{
        jwt::{create_jwt, TokenType},
        mb_to_bytes,
    },
};

use ppdrive_core::{
    config::AppConfig,
    models::{
        bucket::Buckets,
        user::{UserRole, Users},
    },
    options::{CreateBucketOptions, CreateUserOptions},
    tools::secrets::SECRETS_FILENAME,
};

use super::{extractors::ClientRoute, LoginToken, UserLoginViaClient};
mod user;

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    client: ClientRoute,
    Json(data): Json<CreateUserOptions>,
) -> Result<String, AppError> {
    let db = state.db();
    let user_id = Users::create_by_client(db, *client.id(), data).await?;

    Ok(user_id.to_string())
}

#[debug_handler]
async fn login_user(
    State(state): State<AppState>,
    _: ClientRoute,
    Json(data): Json<UserLoginViaClient>,
) -> Result<Json<LoginToken>, AppError> {
    let UserLoginViaClient {
        id,
        access_exp,
        refresh_exp,
    } = data;

    let db = state.db();
    let config = state.config();
    let secrets = state.secrets();

    let user = Users::get_by_pid(db, &id).await?;
    let access_exp = access_exp.unwrap_or(*config.auth().access_exp());
    let refresh_exp = refresh_exp.unwrap_or(*config.auth().refresh_exp());

    let access_token = create_jwt(
        &user.id(),
        secrets.jwt_secret(),
        access_exp,
        TokenType::Access,
    )?;

    let refresh_token = create_jwt(
        &user.id(),
        secrets.jwt_secret(),
        access_exp,
        TokenType::Refresh,
    )?;

    let data = LoginToken {
        access: (access_token, access_exp),
        refresh: (refresh_token, refresh_exp),
    };

    Ok(Json(data))
}

#[debug_handler]
async fn delete_user(
    Path(id): Path<String>,
    _: ClientRoute,
    State(state): State<AppState>,
) -> Result<String, AppError> {
    let db = state.db();
    let user = Users::get_by_pid(db, &id).await?;
    match user.role()? {
        UserRole::Admin => Err(AppError::AuthorizationError(
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
    State(state): State<AppState>,
    client: ClientRoute,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, AppError> {
    let db = state.db();
    if let Some(partition) = &data.partition {
        if partition == SECRETS_FILENAME {
            return Err(AppError::PermissionDenied(
                "partition name {SECRET_FILE} is not allowed".to_string(),
            ));
        }
    }

    let bucket_id = Buckets::create_by_client(db, data, *client.id()).await?;
    Ok(bucket_id.to_string())
}

/// Routes to be requested by PPDRIVE [Client].
pub fn client_routes(config: &AppConfig) -> Router<AppState> {
    let max = config.server().max_upload_size();
    let limit = mb_to_bytes(*max);

    Router::new()
        // Routes used by client for administrative tasks. Requests to these routes
        // require x-ppd-client header.
        .route("/user/register", post(create_user))
        .route("/user/login", post(login_user))
        .route("/user/:id", delete(delete_user))
        // Routes accessible to users created/managed by clients. Requests to these routes
        // do not required x-ppd-client header but may require authorization header
        // if config.auth.url is not provided.
        .route("/user", get(get_user))
        .route("/bucket", post(create_bucket))
        .route("/asset", post(create_asset))
        .layer(DefaultBodyLimit::max(limit))
        .route("/asset/:asset_type/:asset_path", delete(delete_asset))
}

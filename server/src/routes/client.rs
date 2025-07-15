use axum::{
    extract::{Path, State},
    routing::{delete, post},
    Json, Router,
};
use axum_macros::debug_handler;

use crate::{errors::AppError, state::AppState, utils::jwt::create_jwt};

use ppdrive_core::{
    models::{
        bucket::Buckets,
        client::Clients,
        user::{UserRole, Users},
    },
    options::{CreateBucketOptions, CreateUserOptions},
    tools::secrets::SECRETS_FILENAME,
};

use super::{extractors::ClientRoute, LoginCredentials, LoginToken};

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    client_route: ClientRoute,
    Json(data): Json<CreateUserOptions>,
) -> Result<String, AppError> {
    let db = state.db();
    if let Some(partition) = &data.partition {
        if partition == SECRETS_FILENAME {
            return Err(AppError::PermissionDenied(
                "partition name {SECRET_FILE} is not allowed".to_string(),
            ));
        }
    }

    match data.role {
        UserRole::Admin => Err(AppError::InternalServerError(
            "client cannot create admin user".to_string(),
        )),
        _ => {
            let client = Clients::get(db, client_route.pid()).await?;

            let user_id = Users::create_by_client(db, client.id(), data).await?;
            Ok(user_id.to_string())
        }
    }
}

#[debug_handler]
async fn login_user(
    State(state): State<AppState>,
    _: ClientRoute,
    Json(data): Json<LoginCredentials>,
) -> Result<Json<LoginToken>, AppError> {
    let LoginCredentials { id, exp, .. } = data;

    let db = state.db();
    let user = Users::get_by_pid(db, &id).await?;
    let exp = exp.unwrap_or(18_000); // set default expiration to 5 hours

    let config = state.secrets();
    let token = create_jwt(&user.id(), config.jwt_secret(), exp)?;

    let data = LoginToken { token, exp };

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
    client_route: ClientRoute,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, AppError> {
    let db = state.db();
    if let Some(partition) = &data.root_folder {
        if partition == SECRETS_FILENAME {
            return Err(AppError::PermissionDenied(
                "partition name {SECRET_FILE} is not allowed".to_string(),
            ));
        }
    }

    let client = Clients::get(db, client_route.pid()).await?;
    let bucket_id = Buckets::create_by_client(db, client.id(), data).await?;

    Ok(bucket_id.to_string())
}

/// Routes to be requested by PPDRIVE [Client].
pub fn client_routes() -> Router<AppState> {
    Router::new()
        .route("/user/register", post(create_user))
        .route("/user/login", post(login_user))
        .route("/user/:id", delete(delete_user))
        .route("/bucket", post(create_bucket))
}

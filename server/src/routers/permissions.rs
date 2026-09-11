use crate::routers::middlewares::AuthExtractor;
use crate::routers::resp::{api_error, api_response, ApiResponse};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use shared::AssetOwnerName;
use shared::db::{asset, bucket, client, user};
use shared::db::asset::models::PermissionLevel;
use shared::asset_owner_id;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub(crate) struct GrantPermissionRequest {
    /// Relative path of the file within the bucket.
    #[validate(length(min = 1, max = 2048))]
    pub path: String,
    /// PID of the client or email of the user to grant access to.
    #[validate(length(min = 1, max = 255))]
    pub grantee: String,
    /// Type of grantee: "client" or "user". Defaults to "client".
    #[validate(length(min = 1, max = 16))]
    pub grantee_type: Option<String>,
    /// Permission level: read, write, or admin.
    #[validate(length(min = 1, max = 16))]
    pub permission: String,
}

#[derive(Deserialize, Validate)]
pub(crate) struct RevokePermissionRequest {
    /// Relative path of the file within the bucket.
    #[validate(length(min = 1, max = 2048))]
    pub path: String,
    /// PID of the client or email of the user to revoke access from.
    #[validate(length(min = 1, max = 255))]
    pub grantee: String,
    /// Type of grantee: "client" or "user". Defaults to "client".
    #[validate(length(min = 1, max = 16))]
    pub grantee_type: Option<String>,
}

#[derive(Deserialize, Validate)]
pub(crate) struct ListPermissionsQuery {
    /// Optional relative path to filter permissions by a specific file.
    pub path: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct PermissionResponse {
    pub granted: bool,
}

/// Resolve a bucket PID to its data, verify the requesting entity owns it,
/// and return the bucket data.
async fn resolve_owned_bucket(
    state: &AppState,
    bucket_pid: &str,
    auth: &AuthExtractor,
) -> Result<shared::db::bucket::models::Bucket, crate::routers::resp::ResponseError> {
    let bucket_data = bucket::get(bucket_pid, state.db()).await?;

    if bucket_data.public {
        return Err(api_error("permissions are only applicable to private buckets")
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    let owner_id = match auth {
        AuthExtractor::Client(c) => {
            asset_owner_id(AssetOwnerName::Client, c.id(), state.db()).await?
        }
        AuthExtractor::User(u) => {
            let user_id = user::get_id(u.email(), state.db()).await?;
            asset_owner_id(AssetOwnerName::User, user_id, state.db()).await?
        }
    };

    if bucket_data.owner_id != owner_id {
        return Err(api_error("only the bucket owner can manage permissions")
            .with_status_code(StatusCode::FORBIDDEN));
    }

    Ok(bucket_data)
}

/// Resolve a grantee to its asset_owner ID based on type.
async fn resolve_grantee(
    state: &AppState,
    grantee: &str,
    grantee_type: &str,
) -> Result<i32, crate::routers::resp::ResponseError> {
    match grantee_type {
        "user" => {
            let user_id = user::get_id(grantee, state.db()).await?;
            let owner_id = asset_owner_id(AssetOwnerName::User, user_id, state.db()).await?;
            Ok(owner_id)
        }
        _ => {
            let client_id = client::get_id(grantee, state.db()).await?;
            let owner_id = asset_owner_id(AssetOwnerName::Client, client_id, state.db()).await?;
            Ok(owner_id)
        }
    }
}

/// Grant a permission on a file in a private bucket.
///
/// `POST /buckets/{bucket_pid}/permissions`
/// Accepts either client API key or user Bearer token.
#[axum::debug_handler]
pub(crate) async fn grant_permission(
    State(state): State<AppState>,
    Path(bucket_pid): Path<String>,
    auth: AuthExtractor,
    Json(req): Json<GrantPermissionRequest>,
) -> ApiResponse<PermissionResponse> {
    req.validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let grantee_type = req.grantee_type.as_deref().unwrap_or("client");
    let bucket_data = resolve_owned_bucket(&state, &bucket_pid, &auth).await?;
    let grantee_owner_id = resolve_grantee(&state, &req.grantee, grantee_type).await?;

    let permission: PermissionLevel = req.permission.parse()
        .map_err(|e| api_error(e).with_status_code(StatusCode::BAD_REQUEST))?;

    let cleaned_path = req.path.trim_start_matches('/');
    if cleaned_path.is_empty() {
        return Err(api_error("path must not be empty")
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    let asset = asset::get_by_bucket_and_path(state.db(), bucket_data.id, cleaned_path).await?
        .ok_or_else(|| api_error("file not found. Upload the file first to register it.")
            .with_status_code(StatusCode::NOT_FOUND))?;

    asset::grant(state.db(), asset.id, grantee_owner_id, permission).await?;

    tracing::info!(
        bucket_pid = %bucket_pid,
        asset_path = %cleaned_path,
        grantee = %req.grantee,
        grantee_type = %grantee_type,
        permission = %permission,
        "permission granted"
    );

    api_response(PermissionResponse { granted: true })
}

/// Revoke a permission on a file in a private bucket.
///
/// `DELETE /buckets/{bucket_pid}/permissions`
/// Accepts either client API key or user Bearer token.
#[axum::debug_handler]
pub(crate) async fn revoke_permission(
    State(state): State<AppState>,
    Path(bucket_pid): Path<String>,
    auth: AuthExtractor,
    Json(req): Json<RevokePermissionRequest>,
) -> ApiResponse<PermissionResponse> {
    req.validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let grantee_type = req.grantee_type.as_deref().unwrap_or("client");
    let bucket_data = resolve_owned_bucket(&state, &bucket_pid, &auth).await?;
    let grantee_owner_id = resolve_grantee(&state, &req.grantee, grantee_type).await?;

    let cleaned_path = req.path.trim_start_matches('/');
    if cleaned_path.is_empty() {
        return Err(api_error("path must not be empty")
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    let asset = asset::get_by_bucket_and_path(state.db(), bucket_data.id, cleaned_path).await?
        .ok_or_else(|| api_error("file not found")
            .with_status_code(StatusCode::NOT_FOUND))?;

    asset::revoke(state.db(), asset.id, grantee_owner_id).await?;

    tracing::info!(
        bucket_pid = %bucket_pid,
        asset_path = %cleaned_path,
        grantee = %req.grantee,
        grantee_type = %grantee_type,
        "permission revoked"
    );

    api_response(PermissionResponse { granted: false })
}

/// List permissions for files in a private bucket.
///
/// `GET /buckets/{bucket_pid}/permissions?path=<optional>`
/// Accepts either client API key or user Bearer token.
#[axum::debug_handler]
pub(crate) async fn list_permissions(
    State(state): State<AppState>,
    Path(bucket_pid): Path<String>,
    auth: AuthExtractor,
    axum::extract::Query(query): axum::extract::Query<ListPermissionsQuery>,
) -> ApiResponse<Vec<shared::db::asset::models::PermissionWithGrantee>> {
    let bucket_data = resolve_owned_bucket(&state, &bucket_pid, &auth).await?;

    if let Some(ref path) = query.path {
        let cleaned_path = path.trim_start_matches('/');
        let asset = asset::get_by_bucket_and_path(state.db(), bucket_data.id, cleaned_path).await?;
        match asset {
            Some(asset) => {
                let permissions = asset::list_permissions(state.db(), asset.id).await?;
                api_response(permissions)
            }
            None => {
                api_response(vec![])
            }
        }
    } else {
        let permissions = asset::list_all_permissions_for_bucket(state.db(), bucket_data.id).await?;
        api_response(permissions)
    }
}

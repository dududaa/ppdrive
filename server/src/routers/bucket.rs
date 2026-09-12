//! Bucket management endpoints.
//!
//! `POST /buckets` creates a new bucket owned by the authenticated client.

use crate::routers::middlewares::ClientExtractor;
use crate::routers::resp::{api_error, api_response, ApiResponse};
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use shared::db::bucket;
use shared::db::bucket::models::{CreateBucketData, CreateBucketRequest};
use shared::AssetOwnerName;
use std::path::Path;
use validator::Validate;

/// Create a new bucket owned by the authenticated client.
///
/// Validates the request, resolves the client identity, checks path constraints,
/// inserts the bucket row, and creates the directory on disk.
#[axum::debug_handler]
pub(super) async fn create_bucket(
    State(state): State<AppState>,
    client: ClientExtractor,
    Json(req): Json<CreateBucketRequest>,
) -> ApiResponse<String> {
    req.validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    // Validate BEFORE trimming: reject absolute paths and parent-dir traversal
    for component in Path::new(&req.path).components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(
                    api_error("path contains invalid components: '..' is not allowed")
                        .with_status_code(StatusCode::BAD_REQUEST),
                );
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(
                    api_error("path must be relative (must not start with '/')")
                        .with_status_code(StatusCode::BAD_REQUEST),
                );
            }
            _ => {}
        }
    }

    let path = req.path.trim_start_matches('/');
    if path.is_empty() {
        return Err(
            api_error("path must not be empty").with_status_code(StatusCode::BAD_REQUEST),
        );
    }

    let owner_id = client.id();

    let data = CreateBucketData {
        name: req.name,
        path: format!("/{path}"),
        owner_type: AssetOwnerName::Client,
        owner_id,
        public: req.public,
        size: req.size,
        accepts: req.accepts,
    };

    let pid = bucket::create(&data, &state.config().static_folders, state.db()).await?;

    let root_dir = state.config().root_dir()?;
    let bucket_dir = root_dir.join(data.path.trim_start_matches('/'));
    if let Err(err) = tokio::fs::create_dir_all(&bucket_dir).await {
        tracing::error!("failed to create bucket directory: {err}");
        if let Err(del_err) = bucket::delete_by_pid(&pid, state.db()).await {
            tracing::error!("failed to roll back bucket row: {del_err}");
        }
        return Err(api_error("failed to create bucket directory"));
    }

    tracing::info!(bucket_pid = %pid, name = %data.name, path = %data.path, "bucket created");
    Ok(api_response(pid)?.with_status_code(StatusCode::CREATED))
}

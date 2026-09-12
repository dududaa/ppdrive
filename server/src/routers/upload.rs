//! Upload session handlers.
//!
//! Implements `POST /upload/session` (create) and `POST /upload/session/play/{payload}`
//! (play/chunk) endpoints, including path traversal protection, resumable-upload
//! orchestration, and temporary-file management.

use crate::routers::DEFAULT_BODY_LIMIT;
use crate::routers::middlewares::{ClientExtractor, UploadMiddleware};
use crate::routers::resp::{api_error, api_response, ApiResponse};
use crate::state::AppState;
use anyhow::anyhow;
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use shared::server::*;
use shared::{
    db::{asset, bucket, client},
    generate_nano_id,
    root_dir, AssetOwnerName,
};
use shared::asset_owner_id;
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use validator::Validate;
use shared::seconds_from_now;

/// Validate that `user_path` resolves within `root` without path-traversal (`..`).
/// Returns the joined [`PathBuf`] on success.
async fn safe_path(root: &Path, user_path: &str) -> anyhow::Result<PathBuf> {
    let root = root.to_path_buf();
    let user_path = user_path.to_string();

    tokio::task::spawn_blocking(move || {
        let cleaned = user_path.trim_start_matches('/');
        if cleaned.is_empty() {
            return Err(anyhow!("path must not be empty"));
        }

        for component in Path::new(cleaned).components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(anyhow!("path traversal detected: '..' is not allowed"));
            }
        }

        let joined = root.join(cleaned);
        let canonical_root = std::fs::canonicalize(&root)?;

        let canonical_joined = std::fs::canonicalize(&joined).or_else(|_| {
            let parent = joined.parent().unwrap_or(&root);
            std::fs::canonicalize(parent).map(|p| p.join(joined.file_name().unwrap_or_default()))
        })?;

        if !canonical_joined.starts_with(&canonical_root) {
            return Err(anyhow!("path escapes root directory: not allowed"));
        }

        Ok(joined)
    })
    .await?
}

/// Create an upload session and return a signed session token.
///
/// Validates the config, checks bucket ownership, enforces the resumable-upload
/// broker requirement, and signs an [`UploadInfo`] token for the client.
#[axum::debug_handler]
pub(super) async fn create_session(
    State(state): State<AppState>,
    client: ClientExtractor,
    Json(config): Json<UploadUrlConfig>,
) -> ApiResponse<String> {
    config
        .validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    if let Some(bucket_id) = &config.bucket {
        let bucket = bucket::get(bucket_id, state.db()).await?;
        let is_owner = shared::check_ownership(AssetOwnerName::Client, bucket.id, state.db()).await?;

        if !is_owner {
            return Err(api_error("Write access denied for the specified bucket.")
                .with_status_code(StatusCode::FORBIDDEN));
        }

        // Validate MIME type against bucket accepts
        if let Some(ref accepts) = bucket.accepts
            && !accepts.is_empty() {
                let content_type = config.content_type.as_ref().ok_or(
                    api_error("content_type is required for buckets with MIME restrictions")
                        .with_status_code(StatusCode::BAD_REQUEST),
                )?;

                if !bucket::mime_matches_accepts(content_type, accepts) {
                    let list = accepts.join(", ");
                    return Err(api_error(format!(
                        "content_type '{content_type}' is not accepted by this bucket. Accepted: {list}"
                    ))
                    .with_status_code(StatusCode::BAD_REQUEST));
                }
            }
    } else {
        if let AssetType::File = config.asset_type
            && config.accepts.as_ref().is_none_or(|a| a.is_empty()) {
                return Err(api_error("accepts is required when bucket is not provided")
                    .with_status_code(StatusCode::BAD_REQUEST));
            }
    }

    let resumable = config.resumable.unwrap_or_default();
    if resumable && state.config().message_broker.is_none() {
        return Err(
            api_error("Resumable upload is impossible without a message broker.")
                .with_status_code(StatusCode::BAD_REQUEST),
        );
    }

    let mut session_id = None;
    if let AssetType::File = config.asset_type {
        let size = config.target_filesize.ok_or(
            api_error("target_filesize is required for file upload")
                .with_status_code(StatusCode::BAD_REQUEST),
        )?;

        if size >= DEFAULT_BODY_LIMIT as u64 && !config.resumable.unwrap_or_default() {
            return Err(api_error(format!(
                "Files larger than {DEFAULT_BODY_LIMIT} bytes must be resumable."
            ))
            .with_status_code(StatusCode::PAYLOAD_TOO_LARGE));
        }

        // SessionID is tightly coupled with MessageBroker. No need for a session if broker is not provided.
        if resumable && state.config().message_broker.is_some() {
            session_id = Some(generate_nano_id(32));
        }
    }

    let exp = seconds_from_now(config.expires)?;
    let (pid, key) = client::get_claims_data(state.db(), &client.id(), state.secrets()).await?;

    let data = UploadInfo {
        client_id: pid,
        session_id,
        chunk_session_expiration: config.expires,
        config: Some(config),
        chunk_index: 0,
        exp,
        client_key: Some(key.clone()),
    };

    let token = data.sign(&key, state.hasher())?;
    api_response(token)
}

/// Process a chunk or finalize an upload session.
///
/// For files, delegates to [`handle_session`]. For folders, creates the
/// directory (with optional `create_parents`).
#[axum::debug_handler]
pub(super) async fn play_session(
    State(state): State<AppState>,
    UploadMiddleware(mut info): UploadMiddleware,
    body: Bytes,
) -> ApiResponse<Option<String>> {
    if info.config.is_none()
        && let Some(session_id) = &info.session_id
    {
        let cache = state.broker()?.get_upload_info(session_id).await?;
        info.config = cache.config;
    }

    // Limit resumable upload chunk count to prevent abuse
    const MAX_CHUNKS: u16 = 10000;
    if info.chunk_index >= MAX_CHUNKS {
        return Err(api_error(format!("upload exceeded maximum chunk limit of {MAX_CHUNKS}"))
            .with_status_code(StatusCode::PAYLOAD_TOO_LARGE));
    }

    let config = info.config.clone();
    let config = config.ok_or(api_error("missing configuration"))?;
    let root_dir = state.config().root_dir()?;
    let target_path = safe_path(&root_dir, &config.path).await
        .map_err(|e| api_error(e).with_status_code(StatusCode::BAD_REQUEST))?;

    if let Some(bucket_id) = &config.bucket {
        let bucket = bucket::get(bucket_id, state.db()).await?;
        // config.path is the full storage path (e.g. "media/docs/hello.txt").
        // Validate the target is within the bucket's directory.
        let bucket_root = root_dir.join(bucket.path.trim_start_matches('/'));
        let canonical_bucket = std::fs::canonicalize(&bucket_root)
            .map_err(|_| api_error("bucket directory not found").with_status_code(StatusCode::BAD_REQUEST))?;
        let canonical_target = std::fs::canonicalize(&target_path).or_else(|_| {
            let parent = target_path.parent().unwrap_or(&root_dir);
            std::fs::canonicalize(parent).map(|p| p.join(target_path.file_name().unwrap_or_default()))
        }).map_err(|_| api_error("failed to resolve target path").with_status_code(StatusCode::BAD_REQUEST))?;
        if !canonical_target.starts_with(&canonical_bucket) {
            return Err(api_error("upload path is not within the specified bucket")
                .with_status_code(StatusCode::FORBIDDEN));
        }
    }

    let parent_dir = target_path.parent().unwrap_or(&root_dir);
    let target_exists = tokio::fs::metadata(&target_path).await.map(|m| m.is_file()).unwrap_or(false);
    if target_exists && !config.overwrite.unwrap_or_default() {
        return Err(api_error("Asset already exists").with_status_code(StatusCode::CONFLICT));
    }

    let parent_exists = tokio::fs::metadata(parent_dir).await.map(|m| m.is_dir()).unwrap_or(false);
    if parent_dir != root_dir && !parent_exists && !config.create_parents.unwrap_or_default()
    {
        return Err(
            api_error("Parent directory does not exist").with_status_code(StatusCode::NOT_FOUND)
        );
    }

    match config.asset_type {
        AssetType::File => handle_session(&state, info, body, &target_path).await,
        AssetType::Folder => {
            if config.create_parents.unwrap_or_default() {
                tokio::fs::create_dir_all(target_path).await?;
            } else {
                tokio::fs::create_dir(target_path).await?;
            }

            api_response(None)
        }
    }
}

/// Dispatch the upload to [`get_next_session`], cleaning up temp files on failure.
async fn handle_session(
    state: &AppState,
    info: UploadInfo,
    body: Bytes,
    target_path: &PathBuf,
) -> ApiResponse<Option<String>> {
    let session_id = info.session_id.clone();
    match get_next_session(state, info, body, target_path).await {
        Ok(token) => api_response(token),
        Err(err) => {
            if let Some(id) = session_id {
                let tmp_path = root_dir()?.join("tmp").join(id);
                if let Err(err) = tokio::fs::remove_file(tmp_path).await {
                    tracing::error!("unable to clean up file after failure: {err}");
                }
            }

            tracing::error!("upload failed: {err:#}");
            Err(api_error("upload failed"))
        }
    }
}

/// Write chunk data to a temp file, move it to the final path when complete,
/// and return the next resumable session token (or `None`).
async fn get_next_session(
    state: &AppState,
    info: UploadInfo,
    body: Bytes,
    target_path: &PathBuf,
) -> anyhow::Result<Option<String>> {
    let tmp_dir = root_dir()?.join("tmp");
    let root_dir = state.config().root_dir()?;

    let tmp_exists = tokio::fs::metadata(&tmp_dir).await.map(|m| m.is_dir()).unwrap_or(false);
    if !tmp_exists {
        tokio::fs::create_dir(&tmp_dir).await?;
    }

    let config = info
        .config
        .clone()
        .ok_or(anyhow!("missing configuration"))?;

    let session_id = info.session_id.clone();
    let tp_clone = target_path.clone();
    let parent_dir = tp_clone.parent().unwrap_or(&root_dir);

    let target_filesize = config.target_filesize.ok_or(anyhow!(
        "Unable to determine target filesize. Please specify \"target_filesize\" in upload options."
    ))?;

    let resumable = config.resumable.unwrap_or_default();
    let mut next_token = None;

    let (tmp_path, completed) =
        upload_file(session_id.clone(), &tmp_dir, &body, target_filesize).await?;

    if !completed && resumable {
        let session_id = session_id.clone().ok_or(anyhow!("session_id not found"))?;
        let key = info
            .client_key
            .clone()
            .ok_or(anyhow!("client_key not available"))?;
        let mut info = info.clone();

        let broker = state.broker()?;
        broker.upsert_upload_info(&session_id, &info).await?;

        let token = info.resign(&key, state.hasher())?;
        next_token = Some(token);
    }

    if completed {
        match &config.bucket {
            Some(bucket_id) => {
                let bucket = bucket::get(bucket_id, state.db()).await?;
                let bucket_root = root_dir.join(bucket.path.trim_start_matches('/'));

                // validate bucket size
                let is_dir = tokio::fs::metadata(&bucket_root).await.map(|m| m.is_dir()).unwrap_or(false);
                if !is_dir {
                    tokio::fs::create_dir_all(&bucket_root).await?
                } else {
                    if let Some(max_size) = &bucket.size {
                        let mut current_size: u64 = 0;
                        let path_str = bucket_root.to_string_lossy().to_string();
                        shared::get_folder_size(&path_str, &mut current_size).await.unwrap_or(());
                        if current_size + target_filesize > (*max_size as u64) {
                            return Err(anyhow!("Bucket size limit exceeded."));
                        }
                    }
                }

                // Validate MIME type against bucket accepts
                if let Some(ref accepts) = bucket.accepts
                    && !accepts.is_empty() {
                        let inferred_mime = mime_guess::from_path(target_path)
                            .first_or_octet_stream()
                            .to_string();

                        if !bucket::mime_matches_accepts(&inferred_mime, accepts) {
                            // Clean up temp file on validation failure
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            let list = accepts.join(", ");
                            return Err(anyhow!(
                                "file type '{inferred_mime}' is not accepted by this bucket. Accepted: {list}"
                            ));
                        }

                        // Also verify against client-declared content_type if provided
                        if let Some(ref declared) = config.content_type
                            && !bucket::mime_matches_accepts(declared, std::slice::from_ref(&inferred_mime)) {
                                let _ = tokio::fs::remove_file(&tmp_path).await;
                                return Err(anyhow!(
                                    "file MIME type '{inferred_mime}' does not match declared content_type '{declared}'"
                                ));
                            }
                    }
            }
            None => {
                if let Some(ref accepts) = config.accepts
                    && !accepts.is_empty() {
                        let inferred_mime = mime_guess::from_path(target_path)
                            .first_or_octet_stream()
                            .to_string();

                        if !bucket::mime_matches_accepts(&inferred_mime, accepts) {
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            let list = accepts.join(", ");
                            return Err(anyhow!(
                                "file type '{inferred_mime}' is not accepted. Accepted: {list}"
                            ));
                        }
                    }

                if parent_dir != root_dir {
                    let parent_exists = tokio::fs::metadata(parent_dir).await.map(|m| m.is_dir()).unwrap_or(false);
                    if !parent_exists {
                        tokio::fs::create_dir_all(&parent_dir).await?;
                    }
                }
            }
        }

        tokio::fs::rename(tmp_path, target_path).await?;
        if let Some(id) = session_id {
            let broker = state.broker()?;
            broker.remove_upload_info(&id).await?;
        }

        // Register asset and grant admin permission for private bucket files
        if let Some(ref bucket_id_str) = config.bucket {
            let bucket_data = bucket::get(bucket_id_str, state.db()).await?;
            if !bucket_data.public {
                let asset = asset::register(state.db(), bucket_data.id, &config.path).await?;
                let client_numeric_id = client::get_id(&info.client_id, state.db()).await?;
                let owner_id = asset_owner_id(AssetOwnerName::Client, client_numeric_id, state.db()).await?;
                asset::grant(state.db(), asset.id, owner_id, asset::models::PermissionLevel::Admin).await?;
            }
        }

        tracing::info!(path = %config.path, client_id = %info.client_id, "upload completed");
    }

    Ok(next_token)
}

/// Append `data` to a temporary file and report whether the target size is reached.
async fn upload_file(
    session_id: Option<String>,
    tmp_dir: &Path,
    data: &Bytes,
    target_filesize: u64,
) -> anyhow::Result<(PathBuf, bool)> {
    let tmp_name = session_id.unwrap_or(generate_nano_id(32));
    let tmp_path = tmp_dir.join(&tmp_name);

    let mut tmp_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp_path)
        .await?;

    tmp_file.write_all(data).await?;
    tmp_file.flush().await?;

    let tmp_size = tmp_file.metadata().await?.len();
    if tmp_size > target_filesize {
        Err(anyhow!(
            "Uploaded file too large. Expected {target_filesize} bytes. Found {tmp_size} bytes"
        ))
    } else {
        let completed = tmp_size >= target_filesize;
        Ok((tmp_path, completed))
    }
}

//! Upload session handlers.
//!
//! Implements `POST /upload/session` (create) and `POST /upload/session/play/{payload}`
//! (play/chunk) endpoints, including path traversal protection, resumable-upload
//! orchestration, and temporary-file management.

use crate::routers::DEFAULT_BODY_LIMIT;
use crate::routers::middlewares::{ClientExtractor, UploadMiddleware};
use crate::routers::resp::{ApiResponse, ResponseError, api_error, api_response};
use anyhow::anyhow;
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use ppdrive::asset_owner_id;
use ppdrive::seconds_from_now;
use ppdrive::server::*;
use ppdrive::state::AppState;
use ppdrive::{
    AssetOwnerName,
    db::{asset, bucket, client},
    generate_nano_id, root_dir,
};
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use validator::Validate;

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

        // Walk up from the target path until we find an existing ancestor to canonicalize.
        let canonical_joined = {
            let mut attempt = joined.clone();
            loop {
                match std::fs::canonicalize(&attempt) {
                    Ok(canon) => {
                        if attempt != joined {
                            // Re-attach remaining components
                            let stripped = attempt.strip_prefix(&root).unwrap_or(&attempt);
                            let remaining = joined.strip_prefix(stripped).unwrap_or(Path::new(""));
                            break Ok(canon.join(remaining));
                        }
                        break Ok(canon);
                    }
                    Err(_) => match attempt.parent() {
                        Some(parent) if parent != attempt => attempt = parent.to_path_buf(),
                        _ => {
                            break std::fs::canonicalize(joined.parent().unwrap_or(&root))
                                .map(|p| p.join(joined.file_name().unwrap_or_default()));
                        }
                    },
                }
            }
        }?;

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
    let mut config = config;
    config
        .validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    // Transformation first, compression second: when both specify an output
    // format, compression runs last at completion so its rewrite wins.
    if let Some(transformation) = config.image_transformation.clone() {
        transformation
            .validate_operations()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        check_image_plugin("image_transformation", &config).await?;

        // Rewrite only when an output format is requested; `None` keeps the
        // input format (and the existing extension/content_type).
        if let Some(format) = transformation.format {
            let output_mime = format.mime().to_string();
            config.path = format.rewrite_path(&config.path);
            config.content_type = Some(output_mime);
        }
    }

    if let Some(compression) = config.image_compression.clone() {
        check_image_plugin("image_compression", &config).await?;

        // Store the output format: rewrite the path extension and content_type
        // so the overwrite check, MIME validation and asset registration all
        // refer to the file that will actually be written.
        let output_mime = compression.format.mime().to_string();
        config.path = compression.rewrite_path(&config.path);
        config.content_type = Some(output_mime);
    }

    if let Some(bucket_id) = &config.bucket {
        let bucket = bucket::get(bucket_id, state.db()).await?;
        let owner_id = asset_owner_id(AssetOwnerName::Client, client.id(), state.db()).await?;
        let is_owner = bucket.owner_id == owner_id;

        if !is_owner {
            return Err(api_error("Write access denied for the specified bucket.")
                .with_status_code(StatusCode::FORBIDDEN));
        }

        // Validate MIME type against bucket accepts
        if let Some(ref accepts) = bucket.accepts
            && !accepts.is_empty()
        {
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
            && config.content_type.is_none()
        {
            return Err(
                api_error("content_type is required when bucket is not provided")
                    .with_status_code(StatusCode::BAD_REQUEST),
            );
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
        return Err(api_error(format!(
            "upload exceeded maximum chunk limit of {MAX_CHUNKS}"
        ))
        .with_status_code(StatusCode::PAYLOAD_TOO_LARGE));
    }

    let config = info.config.clone();
    let config = config.ok_or(api_error("missing configuration"))?;
    let root_dir = state.config().root_dir()?;
    let target_path = safe_path(&root_dir, &config.path)
        .await
        .map_err(|e| api_error(e).with_status_code(StatusCode::BAD_REQUEST))?;

    if let Some(bucket_id) = &config.bucket {
        let bucket = bucket::get(bucket_id, state.db()).await?;
        let bucket_root = root_dir.join(bucket.path.trim_start_matches('/'));

        // Create bucket directory if it doesn't exist yet
        if !bucket_root.exists() {
            if config.create_parents.unwrap_or_default() {
                tokio::fs::create_dir_all(&bucket_root).await?;
            } else {
                return Err(api_error("bucket directory not found")
                    .with_status_code(StatusCode::BAD_REQUEST));
            }
        }

        // Validate the target path is within the bucket's directory using prefix check
        let canonical_bucket = std::fs::canonicalize(&bucket_root).map_err(|_| {
            api_error("bucket directory not found").with_status_code(StatusCode::BAD_REQUEST)
        })?;
        let canonical_target = std::fs::canonicalize(&target_path)
            .or_else(|_| {
                let parent = target_path.parent().unwrap_or(&root_dir);
                std::fs::canonicalize(parent)
                    .map(|p| p.join(target_path.file_name().unwrap_or_default()))
            })
            .map_err(|_| {
                api_error("failed to resolve target path").with_status_code(StatusCode::BAD_REQUEST)
            })?;
        if !canonical_target.starts_with(&canonical_bucket) {
            return Err(api_error("upload path is not within the specified bucket")
                .with_status_code(StatusCode::FORBIDDEN));
        }
    }

    let parent_dir = target_path.parent().unwrap_or(&root_dir);
    let target_exists = tokio::fs::metadata(&target_path)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false);
    if target_exists && !config.overwrite.unwrap_or_default() {
        return Err(api_error("Asset already exists").with_status_code(StatusCode::CONFLICT));
    }

    let parent_exists = tokio::fs::metadata(parent_dir)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    if parent_dir != root_dir && !parent_exists && !config.create_parents.unwrap_or_default() {
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

    let tmp_exists = tokio::fs::metadata(&tmp_dir)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
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
                let is_dir = tokio::fs::metadata(&bucket_root)
                    .await
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                if !is_dir {
                    tokio::fs::create_dir_all(&bucket_root).await?
                } else {
                    if let Some(max_size) = &bucket.size {
                        let mut current_size: u64 = 0;
                        let path_str = bucket_root.to_string_lossy().to_string();
                        ppdrive::get_folder_size(&path_str, &mut current_size)
                            .await
                            .unwrap_or(());
                        if current_size + target_filesize > (*max_size as u64) {
                            return Err(anyhow!("Bucket size limit exceeded."));
                        }
                    }
                }

                // Validate MIME type against bucket accepts
                if let Some(ref accepts) = bucket.accepts
                    && !accepts.is_empty()
                {
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
                        && !bucket::mime_matches_accepts(
                            declared,
                            std::slice::from_ref(&inferred_mime),
                        )
                    {
                        let _ = tokio::fs::remove_file(&tmp_path).await;
                        return Err(anyhow!(
                            "file MIME type '{inferred_mime}' does not match declared content_type '{declared}'"
                        ));
                    }
                }
            }
            None => {
                // File uploads without a bucket must declare a content_type
                // (enforced at session creation); verify the declaration
                // against the MIME type inferred from the final path.
                let inferred_mime = mime_guess::from_path(target_path)
                    .first_or_octet_stream()
                    .to_string();

                if let Some(ref declared) = config.content_type
                    && !bucket::mime_matches_accepts(declared, std::slice::from_ref(&inferred_mime))
                {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(anyhow!(
                        "file MIME type '{inferred_mime}' does not match declared content_type '{declared}'"
                    ));
                }

                if parent_dir != root_dir {
                    let parent_exists = tokio::fs::metadata(parent_dir)
                        .await
                        .map(|m| m.is_dir())
                        .unwrap_or(false);
                    if !parent_exists {
                        tokio::fs::create_dir_all(&parent_dir).await?;
                    }
                }
            }
        }

        let transformation = config.image_transformation.clone();
        let compression = config.image_compression.clone();
        // One flag for the whole post-processing chain: if either option asks
        // for background handling, transform → compress → rename → register
        // all run after the response (order matters for output formats).
        let background = transformation
            .as_ref()
            .map(|t| {
                resolve_background(t.background, state.config().image_transformation_background)
            })
            .unwrap_or(false)
            || compression
                .as_ref()
                .map(|c| {
                    resolve_background(c.background, state.config().image_compression_background)
                })
                .unwrap_or(false);

        if background {
            // All bytes are in and validations passed; drop the session now
            // and finalize (transform → compress → rename → register) off the request.
            if let Some(id) = session_id {
                let broker = state.broker()?;
                broker.remove_upload_info(&id).await?;
            }

            let state = state.clone();
            let config = config.clone();
            let client_id = info.client_id.clone();
            let tmp_path = tmp_path.clone();
            let target_path = target_path.clone();
            tokio::spawn(async move {
                finalize_in_background(
                    state,
                    config,
                    client_id,
                    tmp_path,
                    target_path,
                    transformation,
                    compression,
                )
                .await;
            });
        } else {
            apply_post_processing(&tmp_path, transformation.as_ref(), compression.as_ref()).await?;

            tokio::fs::rename(&tmp_path, target_path).await?;
            if let Some(id) = session_id {
                let broker = state.broker()?;
                broker.remove_upload_info(&id).await?;
            }

            register_asset(state, &config, &info.client_id).await?;
            tracing::info!(path = %config.path, client_id = %info.client_id, "upload completed");
        }
    }

    Ok(next_token)
}

/// Shared preconditions for the image processing options: file uploads only,
/// a declared `image/*` `content_type`, and the matching plugin installed.
///
/// `option` is the upload config field (snake_case, e.g. `image_compression`);
/// the plugin id follows the hyphen convention (`image-compression`).
async fn check_image_plugin(option: &str, config: &UploadUrlConfig) -> Result<(), ResponseError> {
    let plugin_id = option.replace('_', "-");

    if !matches!(config.asset_type, AssetType::File) {
        return Err(api_error(format!("{option} only applies to file uploads"))
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    let content_type = config.content_type.as_deref().ok_or(
        api_error(format!("content_type is required when {option} is set"))
            .with_status_code(StatusCode::BAD_REQUEST),
    )?;
    if !content_type.starts_with("image/") {
        return Err(api_error(format!(
            "{option} requires content_type to be an image/* type, got '{content_type}'"
        ))
        .with_status_code(StatusCode::BAD_REQUEST));
    }

    if crate::app::find_plugin(&plugin_id).await.is_none() {
        return Err(api_error(format!("{plugin_id} plugin is not installed"))
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    Ok(())
}

/// Apply the configured post-processing — transformation first, then
/// compression — to the completed upload in place.
async fn apply_post_processing(
    path: &Path,
    transformation: Option<&ImageTransformationConfig>,
    compression: Option<&ImageCompressionConfig>,
) -> anyhow::Result<()> {
    if let Some(transformation) = transformation {
        apply_media_plugin("image-transformation", path, transformation).await?;
    }
    if let Some(compression) = compression {
        apply_media_plugin("image-compression", path, compression).await?;
    }
    Ok(())
}

/// Run a media plugin over the completed upload in place.
///
/// The full assembled file is read from `path` (safe for resumable uploads:
/// only runs once every chunk is received), processed on the blocking pool,
/// and written back to `path`.
async fn apply_media_plugin(
    plugin_id: &str,
    path: &Path,
    options: &impl serde::Serialize,
) -> anyhow::Result<()> {
    let plugin = crate::app::find_plugin(plugin_id)
        .await
        .ok_or_else(|| anyhow!("{plugin_id} plugin is not installed"))?;

    let input = tokio::fs::read(path).await?;
    let options_json = serde_json::to_value(options)?;
    let dest = path.to_path_buf();
    let plugin_id_owned = plugin_id.to_string();

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut dispatcher = ppdrive::plugin::loader::PluginDispatcher::<Vec<u8>>::new();
        let output = dispatcher
            .dispatch(plugin, (input.as_slice(), &options_json))
            .map_err(|err| anyhow!("{plugin_id_owned} failed: {err}"))?;
        std::fs::write(&dest, output)
            .map_err(|err| anyhow!("failed to write processed file: {err}"))?;
        Ok(())
    })
    .await??;

    tracing::info!(plugin = plugin_id, path = %path.display(), "upload post-processed");
    Ok(())
}

/// Finalize an upload after the success response has been sent.
/// Failures are logged only (and the temp file cleaned up) — the client
/// can no longer learn about them.
async fn finalize_in_background(
    state: AppState,
    config: UploadUrlConfig,
    client_id: String,
    tmp_path: PathBuf,
    target_path: PathBuf,
    transformation: Option<ImageTransformationConfig>,
    compression: Option<ImageCompressionConfig>,
) {
    let result = async {
        apply_post_processing(&tmp_path, transformation.as_ref(), compression.as_ref()).await?;
        tokio::fs::rename(&tmp_path, &target_path).await?;
        register_asset(&state, &config, &client_id).await?;
        anyhow::Ok(())
    }
    .await;

    match result {
        Ok(()) => tracing::info!(
            path = %config.path,
            client_id = %client_id,
            "upload completed (background)"
        ),
        Err(err) => {
            tracing::error!(path = %config.path, "background upload finalization failed: {err:#}");
            let _ = tokio::fs::remove_file(&tmp_path).await;
        }
    }
}

/// Register the asset and grant admin permission for private bucket files.
async fn register_asset(
    state: &AppState,
    config: &UploadUrlConfig,
    client_id: &str,
) -> anyhow::Result<()> {
    let Some(bucket_id_str) = &config.bucket else {
        return Ok(());
    };

    let bucket_data = bucket::get(bucket_id_str, state.db()).await?;
    if bucket_data.public {
        return Ok(());
    }

    let bucket_prefix = bucket_data
        .path
        .trim_start_matches('/')
        .trim_end_matches('/');
    let asset_path = config
        .path
        .trim_start_matches('/')
        .strip_prefix(bucket_prefix)
        .unwrap_or(&config.path)
        .trim_start_matches('/');
    let asset = asset::register(state.db(), bucket_data.id, asset_path).await?;
    let client_numeric_id = client::get_id(client_id, state.db()).await?;
    let owner_id = asset_owner_id(AssetOwnerName::Client, client_numeric_id, state.db()).await?;
    asset::grant(
        state.db(),
        asset.id,
        owner_id,
        asset::models::PermissionLevel::Admin,
    )
    .await?;
    Ok(())
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

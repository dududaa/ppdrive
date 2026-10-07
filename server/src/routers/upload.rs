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
        // A fresh install has no storage directory yet; create it on
        // demand so session creation does not fail with a raw io error.
        let canonical_root = match std::fs::canonicalize(&root) {
            Ok(canonical) => canonical,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(&root)?;
                std::fs::canonicalize(&root)?
            }
            Err(err) => return Err(err.into()),
        };

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

    // Transformation first, conversion second: when both specify an output
    // format, conversion runs last at completion so its rewrite wins.
    // The audio and video options follow the same rule — transformation
    // before conversion — and can never combine across families: each
    // requires its own `content_type` family, so a session validates for at
    // most one of them. `media_streaming` packages whatever the chain
    // produced, after the rename: it accepts `video/*`/`audio/*` sources,
    // so it may combine with the audio or video options (never with the
    // image ones).
    if let Some(transformation) = config.image_transformation.clone() {
        transformation
            .validate_operations()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        check_media_plugin("image_transformation", &config).await?;

        // Rewrite only when an output format is requested; `None` keeps the
        // input format (and the existing extension/content_type).
        if let Some(format) = transformation.format {
            let output_mime = format.mime().to_string();
            config.path = format.rewrite_path(&config.path);
            config.content_type = Some(output_mime);
        }
    }

    if let Some(conversion) = config.image_conversion.clone() {
        conversion
            .validate_scale()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        check_media_plugin("image_conversion", &config).await?;

        // Store the output format: rewrite the path extension and content_type
        // so the overwrite check, MIME validation and asset registration all
        // refer to the file that will actually be written.
        let output_mime = conversion.format.mime().to_string();
        config.path = conversion.rewrite_path(&config.path);
        config.content_type = Some(output_mime);
    }

    if let Some(effects) = config.audio_effects.clone() {
        effects
            .validate_operations()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        check_media_plugin("audio_effects", &config).await?;

        // The plugin always re-encodes — an omitted format writes WAV —
        // so the path/content_type are rewritten to the output format
        // either way (a later audio_conversion rewrite wins).
        let output_mime = effects.output_format().mime().to_string();
        config.path = effects.rewrite_path(&config.path);
        config.content_type = Some(output_mime);
    }

    if let Some(conversion) = config.audio_conversion.clone() {
        check_media_plugin("audio_conversion", &config).await?;

        // Store the output format: rewrite the path extension and content_type
        // so the overwrite check, MIME validation and asset registration all
        // refer to the file that will actually be written.
        let output_mime = conversion.format.mime().to_string();
        config.path = conversion.rewrite_path(&config.path);
        config.content_type = Some(output_mime);
    }

    if let Some(transformation) = config.video_transformation.clone() {
        transformation
            .validate_operations()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        check_media_plugin("video_transformation", &config).await?;

        // Rewrite only when an output format is requested; `None` keeps the
        // input container (and the existing extension/content_type).
        if let Some(format) = transformation.format {
            let output_mime = format.mime().to_string();
            config.path = format.rewrite_path(&config.path);
            config.content_type = Some(output_mime);
        }
    }

    if let Some(conversion) = config.video_conversion.clone() {
        conversion
            .validate_scale()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        check_media_plugin("video_conversion", &config).await?;

        // Store the output format: rewrite the path extension and content_type
        // so the overwrite check, MIME validation and asset registration all
        // refer to the file that will actually be written.
        let output_mime = conversion.format.mime().to_string();
        config.path = conversion.rewrite_path(&config.path);
        config.content_type = Some(output_mime);
    }

    // Streaming runs last at completion — after the in-place chain and the
    // rename — but its checks sit with the other option gates so a missing
    // `content_type` reports the option (not the bucket) as the requirement.
    if let Some(streaming) = config.media_streaming.clone() {
        streaming
            .validate_renditions()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
        if let Some(output_dir) = &streaming.output_dir {
            validate_streaming_output_dir(&state, &config, output_dir).await?;
        }
        check_media_plugin("media_streaming", &config).await?;
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

        // The target rarely exists yet on a first upload — canonicalize_loose
        // walks to the nearest existing ancestor and re-attaches the missing
        // components, so a not-yet-created file or parent directory does not
        // fail the containment check before create_parents has run.
        let canonical_target = canonicalize_loose(&target_path).map_err(|err| {
            tracing::error!(
                "failed to resolve upload target {}: {err}",
                target_path.display()
            );
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

    let mut config = info
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
        // The play handler guarantees the parent exists when create_parents
        // is off (404 otherwise), so a missing parent here means it was
        // requested: create it before the rename — bucket uploads reach the
        // rename too, and previously only non-bucket uploads created parents.
        if parent_dir != root_dir {
            let parent_exists = tokio::fs::metadata(parent_dir)
                .await
                .map(|m| m.is_dir())
                .unwrap_or(false);
            if !parent_exists {
                tokio::fs::create_dir_all(&parent_dir).await?;
            }
        }

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
            }
        }

        let post = PostProcessing {
            image_transformation: config.image_transformation.clone(),
            image_conversion: config.image_conversion.clone(),
            audio_effects: config.audio_effects.clone(),
            audio_conversion: config.audio_conversion.clone(),
            video_transformation: config.video_transformation.clone(),
            video_conversion: config.video_conversion.clone(),
        };
        // One flag for the whole post-processing chain: if any option asks
        // for background handling, processing → rename → package → register
        // all run after the response (order matters for output formats).
        let background = {
            let global = state.config();
            post.background(global)
                || config
                    .media_streaming
                    .as_ref()
                    .map(|streaming| {
                        resolve_background(streaming.background, global.media_streaming_background)
                    })
                    .unwrap_or(false)
        };

        if background {
            // All bytes are in and validations passed; drop the session now
            // and finalize (process → rename → register) off the request.
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
                finalize_in_background(state, config, client_id, tmp_path, target_path, post).await;
            });
        } else {
            post.apply(&tmp_path).await?;

            tokio::fs::rename(&tmp_path, target_path).await?;
            if let Some(id) = session_id {
                let broker = state.broker()?;
                broker.remove_upload_info(&id).await?;
            }

            apply_media_streaming(state, &mut config, target_path).await?;
            register_asset(state, &config, &info.client_id).await?;
            tracing::info!(path = %config.path, client_id = %info.client_id, "upload completed");
        }
    }

    Ok(next_token)
}

/// The post-processing options configured for one upload session, applied
/// in place to the completed file.
///
/// At most one in-place family is ever populated: the session-creation
/// checks require either an `image/*` or an `audio/*` `content_type`,
/// never both. `media_streaming` is not part of this struct — it packages
/// the file after the rename, once it sits at its target path.
#[derive(Clone, Default)]
struct PostProcessing {
    image_transformation: Option<ImageTransformationConfig>,
    image_conversion: Option<ImageConversionConfig>,
    audio_effects: Option<AudioEffectsConfig>,
    audio_conversion: Option<AudioConversionConfig>,
    video_transformation: Option<VideoTransformationConfig>,
    video_conversion: Option<VideoConversionConfig>,
}

impl PostProcessing {
    /// Whether the chain runs after the response: any option's
    /// `background` (client) or its global `ppd_config.toml` default
    /// asks for it.
    fn background(&self, global: &ppdrive::config::AppConfig) -> bool {
        self.image_transformation
            .as_ref()
            .map(|t| resolve_background(t.background, global.image_transformation_background))
            .unwrap_or(false)
            || self
                .image_conversion
                .as_ref()
                .map(|c| resolve_background(c.background, global.image_conversion_background))
                .unwrap_or(false)
            || self
                .audio_effects
                .as_ref()
                .map(|e| resolve_background(e.background, global.audio_effects_background))
                .unwrap_or(false)
            || self
                .audio_conversion
                .as_ref()
                .map(|c| resolve_background(c.background, global.audio_conversion_background))
                .unwrap_or(false)
            || self
                .video_transformation
                .as_ref()
                .map(|t| resolve_background(t.background, global.video_transformation_background))
                .unwrap_or(false)
            || self
                .video_conversion
                .as_ref()
                .map(|c| resolve_background(c.background, global.video_conversion_background))
                .unwrap_or(false)
    }

    /// Apply the configured post-processing to `path` in place, each
    /// family in chain order: image transform → conversion, audio
    /// effects → conversion, video transform → conversion.
    async fn apply(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(transformation) = &self.image_transformation {
            apply_media_plugin("image-transformation", path, transformation).await?;
        }
        if let Some(conversion) = &self.image_conversion {
            apply_media_plugin("image-conversion", path, conversion).await?;
        }
        if let Some(effects) = &self.audio_effects {
            apply_media_plugin("audio-effects", path, effects).await?;
        }
        if let Some(conversion) = &self.audio_conversion {
            apply_media_plugin("audio-conversion", path, conversion).await?;
        }
        if let Some(transformation) = &self.video_transformation {
            apply_media_plugin("video-transformation", path, transformation).await?;
        }
        if let Some(conversion) = &self.video_conversion {
            apply_media_plugin("video-conversion", path, conversion).await?;
        }
        Ok(())
    }
}

/// Shared preconditions for the media processing options: file uploads only,
/// a declared `<media>/*` `content_type`, and the matching plugin installed.
///
/// `option` is the upload config field (snake_case, e.g. `image_conversion`);
/// the media prefix (`image`, `audio`, `video`) and the plugin id
/// (`image-conversion`) follow from it. The `media_streaming` option is
/// the one exception: its `media` prefix maps to the plugin id
/// (`media-streaming`) but its accepted families are `video/*` and
/// `audio/*` — whatever FFmpeg can probe.
async fn check_media_plugin(option: &str, config: &UploadUrlConfig) -> Result<(), ResponseError> {
    let plugin_id = option.replace('_', "-");
    let media = option.split('_').next().unwrap_or_default();

    if !matches!(config.asset_type, AssetType::File) {
        return Err(api_error(format!("{option} only applies to file uploads"))
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    let content_type = config.content_type.as_deref().ok_or(
        api_error(format!("content_type is required when {option} is set"))
            .with_status_code(StatusCode::BAD_REQUEST),
    )?;
    let (family_ok, expected) = if media == "media" {
        (
            content_type.starts_with("video/") || content_type.starts_with("audio/"),
            "a video/* or audio/*".to_string(),
        )
    } else {
        let article = if media.starts_with(['a', 'e', 'i', 'o', 'u']) {
            "an"
        } else {
            "a"
        };
        (
            content_type.starts_with(&format!("{media}/")),
            format!("{article} {media}/*"),
        )
    };
    if !family_ok {
        return Err(api_error(format!(
            "{option} requires content_type to be {expected} type, got '{content_type}'"
        ))
        .with_status_code(StatusCode::BAD_REQUEST));
    }

    if let Err(message) = crate::app::require_plugin(&plugin_id).await {
        return Err(api_error(message).with_status_code(StatusCode::BAD_REQUEST));
    }

    Ok(())
}

/// Canonicalize `path`, tolerating components that do not exist yet:
/// the nearest existing ancestor is canonicalized and the remaining
/// components are re-attached (the walk [`safe_path`] performs against
/// the server root, generalized to any root).
fn canonicalize_loose(path: &Path) -> anyhow::Result<PathBuf> {
    let mut attempt = path.to_path_buf();
    loop {
        match std::fs::canonicalize(&attempt) {
            Ok(canon) => {
                if attempt == path {
                    return Ok(canon);
                }
                let remaining = path.strip_prefix(&attempt).unwrap_or(Path::new(""));
                return Ok(canon.join(remaining));
            }
            Err(err) => match attempt.parent() {
                Some(parent) if parent != attempt => attempt = parent.to_path_buf(),
                _ => {
                    return Err(anyhow!("failed to resolve path {}: {err}", path.display()));
                }
            },
        }
    }
}

/// Resolve `media_streaming.output_dir` at session creation and enforce
/// the same containment rules as the upload path: no traversal (via
/// [`safe_path`], rooted at the server root) and — with a bucket — inside
/// the bucket directory. The package directory must also not contain the
/// upload path, or packaging would remove the source before reading it.
async fn validate_streaming_output_dir(
    state: &AppState,
    config: &UploadUrlConfig,
    output_dir: &str,
) -> Result<(), ResponseError> {
    let root_dir = state.config().root_dir().map_err(|err| {
        api_error(format!("failed to resolve root directory: {err}"))
            .with_status_code(StatusCode::INTERNAL_SERVER_ERROR)
    })?;
    let out_path = safe_path(&root_dir, output_dir)
        .await
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let target_path = root_dir.join(config.path.trim_start_matches('/'));
    if target_path.starts_with(&out_path) {
        return Err(
            api_error("media_streaming output_dir must not contain the upload path")
                .with_status_code(StatusCode::BAD_REQUEST),
        );
    }

    if let Some(bucket_id) = &config.bucket {
        let bucket = bucket::get(bucket_id, state.db()).await?;
        let bucket_root = root_dir.join(bucket.path.trim_start_matches('/'));
        if !out_path.starts_with(&bucket_root) {
            return Err(
                api_error("media_streaming output_dir is not within the specified bucket")
                    .with_status_code(StatusCode::BAD_REQUEST),
            );
        }
        if bucket_root.exists() {
            let canonical_bucket = std::fs::canonicalize(&bucket_root).map_err(|err| {
                api_error(format!("failed to resolve bucket directory: {err}"))
                    .with_status_code(StatusCode::BAD_REQUEST)
            })?;
            let canonical_out = canonicalize_loose(&out_path)
                .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;
            if !canonical_out.starts_with(&canonical_bucket) {
                return Err(api_error(
                    "media_streaming output_dir is not within the specified bucket",
                )
                .with_status_code(StatusCode::BAD_REQUEST));
            }
        }
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
    let input = tokio::fs::read(path).await?;
    let output = run_media_plugin(plugin_id, input, options).await?;
    tokio::fs::write(path, output).await?;

    tracing::info!(plugin = plugin_id, path = %path.display(), "upload post-processed");
    Ok(())
}

/// Look up a media plugin and run it over `input` bytes on the blocking pool,
/// returning the processed bytes.
///
/// Shared by upload post-processing and on-the-fly download transformation.
pub(crate) async fn run_media_plugin(
    plugin_id: &str,
    input: Vec<u8>,
    options: &impl serde::Serialize,
) -> anyhow::Result<Vec<u8>> {
    let plugin = crate::app::require_plugin(plugin_id)
        .await
        .map_err(anyhow::Error::msg)?;
    let options_json = serde_json::to_value(options)?;
    let plugin_id_owned = plugin_id.to_string();

    tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let mut dispatcher = ppdrive::plugin::loader::PluginDispatcher::<Vec<u8>>::new();
        let output = dispatcher
            .dispatch(plugin, (input.as_slice(), &options_json))
            .map_err(|err| anyhow!("{plugin_id_owned} failed: {err}"))?;
        Ok(output.clone())
    })
    .await?
}

/// Look up the `media-streaming` plugin and package `input` into
/// `output_dir` on the blocking pool, returning the plugin's JSON
/// description of the package (`{"playlist": ..., "files": [...]}`).
///
/// Unlike the byte-oriented plugins, this dispatch is file-based: the
/// plugin reads the input path itself and writes many files.
pub(crate) async fn run_media_streaming_plugin(
    plugin_id: &str,
    input: &Path,
    output_dir: &Path,
    options: &impl serde::Serialize,
) -> anyhow::Result<Vec<u8>> {
    let plugin = crate::app::require_plugin(plugin_id)
        .await
        .map_err(anyhow::Error::msg)?;
    let options_json = serde_json::to_value(options)?;
    let plugin_id_owned = plugin_id.to_string();
    let input = input.to_path_buf();
    let output_dir = output_dir.to_path_buf();

    tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let mut dispatcher = ppdrive::plugin::loader::PluginDispatcher::<Vec<u8>>::new();
        let output = dispatcher
            .dispatch(
                plugin,
                (input.as_path(), output_dir.as_path(), &options_json),
            )
            .map_err(|err| anyhow!("{plugin_id_owned} failed: {err}"))?;
        Ok(output.clone())
    })
    .await?
}

/// Package the completed upload into an HLS/DASH stream directory.
///
/// Runs after the file is renamed into place: the `media-streaming`
/// plugin is file-based (input path + output directory in, playlist JSON
/// out), one call producing many files. The package is written to
/// `media_streaming.output_dir` (the same bucket rules apply) or, when
/// omitted, a sidecar directory beside the file (`<path>.stream`); an
/// existing package directory is replaced. With `delete_source` (the
/// default) the source file is removed once the package is written and
/// the registered asset path becomes the master playlist.
async fn apply_media_streaming(
    state: &AppState,
    config: &mut UploadUrlConfig,
    target_path: &Path,
) -> anyhow::Result<()> {
    let Some(streaming) = config.media_streaming.clone() else {
        return Ok(());
    };
    let root_dir = state.config().root_dir()?;

    let out_dir = match &streaming.output_dir {
        Some(output_dir) => safe_path(&root_dir, output_dir).await?,
        None => {
            let file_name = target_path
                .file_name()
                .ok_or_else(|| anyhow!("upload path has no file name"))?;
            let mut sidecar = file_name.to_os_string();
            sidecar.push(".stream");
            target_path.with_file_name(sidecar)
        }
    };

    // Defense in depth: packaging into a directory that contains the
    // source would wipe it before the plugin ever reads it.
    if target_path.starts_with(&out_dir) {
        return Err(anyhow!(
            "media_streaming output_dir must not contain the upload path"
        ));
    }

    if out_dir.exists() {
        tokio::fs::remove_dir_all(&out_dir).await?;
    }
    tokio::fs::create_dir_all(&out_dir).await?;

    if let Some(bucket_id) = &config.bucket {
        let bucket = bucket::get(bucket_id, state.db()).await?;
        let bucket_root = root_dir.join(bucket.path.trim_start_matches('/'));
        let canonical_bucket = std::fs::canonicalize(&bucket_root)
            .map_err(|_| anyhow!("bucket directory not found"))?;
        let canonical_out = std::fs::canonicalize(&out_dir)
            .map_err(|err| anyhow!("failed to resolve output_dir: {err}"))?;
        if !canonical_out.starts_with(&canonical_bucket) {
            let _ = tokio::fs::remove_dir_all(&out_dir).await;
            return Err(anyhow!(
                "media_streaming output_dir is not within the specified bucket"
            ));
        }
    }

    let payload =
        run_media_streaming_plugin("media-streaming", target_path, &out_dir, &streaming).await?;
    let playlist = {
        let value: serde_json::Value = serde_json::from_slice(&payload)?;
        value
            .get("playlist")
            .and_then(|playlist| playlist.as_str())
            .ok_or_else(|| anyhow!("media-streaming returned no playlist"))?
            .to_string()
    };

    if streaming.delete_source {
        let playlist_name = Path::new(&playlist)
            .file_name()
            .ok_or_else(|| anyhow!("media-streaming returned an invalid playlist path"))?;
        tokio::fs::remove_file(target_path).await?;
        let mut playlist_rel = out_dir
            .strip_prefix(&root_dir)
            .map_err(|_| anyhow!("output_dir is outside the server root"))?
            .to_path_buf();
        playlist_rel.push(playlist_name);
        config.path = playlist_rel.to_string_lossy().replace('\\', "/");
    }

    tracing::info!(
        plugin = "media-streaming",
        output_dir = %out_dir.display(),
        source_deleted = streaming.delete_source,
        "upload packaged for streaming"
    );
    Ok(())
}

/// Finalize an upload after the success response has been sent.
/// Failures are logged only (and the temp file cleaned up) — the client
/// can no longer learn about them.
async fn finalize_in_background(
    state: AppState,
    mut config: UploadUrlConfig,
    client_id: String,
    tmp_path: PathBuf,
    target_path: PathBuf,
    post: PostProcessing,
) {
    let result = async {
        post.apply(&tmp_path).await?;
        tokio::fs::rename(&tmp_path, &target_path).await?;
        apply_media_streaming(&state, &mut config, &target_path).await?;
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

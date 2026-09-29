//! Download handlers for private bucket file access.
//!
//! `POST /download/sign` generates a time-limited signed token for a file.
//! `GET /download/{token}` serves the file with Range-header support.

use crate::routers::middlewares::{ClientExtractor, DownloadMiddleware};
use crate::routers::resp::{ApiResponse, ResponseError, api_error, api_response};
use crate::routers::upload::run_media_plugin;
use axum::Json;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use ppdrive::AssetOwnerName;
use ppdrive::asset_owner_id;
use ppdrive::db::asset::models::PermissionLevel;
use ppdrive::db::{asset, bucket, client};
use ppdrive::seconds_from_now;
use ppdrive::server::{DownloadInfo, ImageTransformationConfig, SignDownloadRequest};
use ppdrive::state::AppState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use validator::Validate;

/// Plugin applied for on-the-fly download transformations.
const TRANSFORM_PLUGIN: &str = "image-transformation";
/// Default TTL for broker-cached transformed outputs (24h).
const TRANSFORM_CACHE_TTL_DEFAULT: u64 = 86_400;
/// Default `Cache-Control: max-age` for transformed responses (1h).
const TRANSFORM_CACHE_MAX_AGE_DEFAULT: u64 = 3_600;

/// Resolve the file path for a download request.
///
/// Fetches the bucket by PID, joins it with the relative path from the token,
/// canonicalizes the result, and verifies it falls within the storage root.
async fn resolve_file_path(
    state: &AppState,
    bucket_pid: &str,
    relative_path: &str,
) -> Result<std::path::PathBuf, ResponseError> {
    let bucket = bucket::get(bucket_pid, state.db()).await?;
    let root_dir = state.config().root_dir()?;
    let bucket_root = root_dir.join(bucket.path.trim_start_matches('/'));

    for component in std::path::Path::new(relative_path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(
                api_error("path contains invalid components: '..' is not allowed")
                    .with_status_code(StatusCode::BAD_REQUEST),
            );
        }
    }

    let file_path = bucket_root.join(relative_path);

    let (canonical_file, is_file) = tokio::task::spawn_blocking(move || {
        let canonical_root = std::fs::canonicalize(&root_dir)?;

        let canonical_file = std::fs::canonicalize(&file_path).or_else(|_| {
            let parent = file_path.parent().unwrap_or(&bucket_root);
            std::fs::canonicalize(parent).map(|p| p.join(file_path.file_name().unwrap_or_default()))
        })?;

        if !canonical_file.starts_with(&canonical_root) {
            return Err(anyhow::anyhow!("path escapes storage root"));
        }

        let is_file = canonical_file.is_file();
        Ok::<_, anyhow::Error>((canonical_file, is_file))
    })
    .await
    .map_err(|e| api_error(format!("failed to resolve path: {e}")))?
    .map_err(|e| api_error(format!("failed to resolve path: {e}")))?;

    if !is_file {
        return Err(api_error("file not found").with_status_code(StatusCode::NOT_FOUND));
    }

    Ok(canonical_file)
}

/// Parse a `Range: bytes=START-END` header into (start, end) byte offsets.
///
/// Returns `None` if the header is absent or malformed.
fn parse_range(header: &str, file_size: u64) -> Option<(u64, u64)> {
    let header = header.strip_prefix("bytes=")?;
    let (start_str, end_str) = header.split_once('-')?;
    if start_str.is_empty() {
        // Suffix range: bytes=-N (last N bytes)
        let n: u64 = end_str.parse().ok()?;
        if n == 0 || n > file_size {
            return None;
        }
        Some((file_size - n, file_size - 1))
    } else {
        let start: u64 = start_str.parse().ok()?;
        let end = if end_str.is_empty() {
            file_size - 1
        } else {
            end_str.parse().ok()?
        };
        if start > end || start >= file_size {
            return None;
        }
        Some((start, end))
    }
}

/// Nanoseconds since the Unix epoch for a file's last modification.
fn mtime_nanos(metadata: &std::fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|age| age.as_nanos())
        .unwrap_or(0)
}

/// Whether an `If-None-Match` header (single value, list, or `*`) matches `etag`.
fn etag_matches(if_none_match: Option<&HeaderValue>, etag: &str) -> bool {
    let Some(value) = if_none_match.and_then(|value| value.to_str().ok()) else {
        return false;
    };
    value.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || candidate.trim_matches('"') == etag
    })
}

/// `ETag` + `Cache-Control` headers shared by transformed responses.
fn cache_headers(etag: &str, max_age: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&format!("\"{etag}\"")) {
        headers.insert(header::ETAG, value);
    }
    if let Ok(value) = HeaderValue::from_str(&format!("public, max-age={max_age}")) {
        headers.insert(header::CACHE_CONTROL, value);
    }
    headers
}

/// Serve transformed bytes as a full `200` response.
fn transformed_response(bytes: Vec<u8>, mime: &str, etag: &str, max_age: u64) -> Response {
    let mut headers = cache_headers(etag, max_age);
    let mime_value = HeaderValue::from_str(mime)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, mime_value);
    (StatusCode::OK, headers, bytes).into_response()
}

/// Serve the transformed rendition of a source file.
///
/// Shared by signed and direct downloads. Order: `304` revalidation (cheap —
/// the key covers source + options, so no plugin or cache is needed) → broker
/// cache hit → plugin check → transform and store.
///
/// `max_age_ceiling` caps `Cache-Control` so signed-download responses never
/// advertise a lifetime beyond the token's expiry.
pub(crate) async fn serve_transformed(
    state: &AppState,
    request_headers: &HeaderMap,
    file_path: &Path,
    size: u64,
    mtime: u128,
    config: &ImageTransformationConfig,
    max_age_ceiling: Option<u64>,
) -> Result<Response, ResponseError> {
    let etag = config.cache_key(&file_path.to_string_lossy(), size, mtime);

    let configured_max_age = state
        .config()
        .transform_cache_max_age_secs
        .unwrap_or(TRANSFORM_CACHE_MAX_AGE_DEFAULT);
    let max_age = max_age_ceiling.map_or(configured_max_age, |ceiling| {
        configured_max_age.min(ceiling)
    });

    if etag_matches(request_headers.get(header::IF_NONE_MATCH), &etag) {
        return Ok((StatusCode::NOT_MODIFIED, cache_headers(&etag, max_age)).into_response());
    }

    let ttl = state
        .config()
        .transform_cache_ttl_secs
        .unwrap_or(TRANSFORM_CACHE_TTL_DEFAULT);
    let cache = if ttl > 0 { state.broker().ok() } else { None };

    let source_mime = mime_guess::from_path(file_path)
        .first_or_octet_stream()
        .to_string();
    let output_mime = config
        .format
        .map(|format| format.mime().to_string())
        .unwrap_or_else(|| source_mime.clone());

    if let Some(broker) = cache {
        match broker.get_transform_cache(&etag).await {
            Ok(Some(bytes)) => {
                return Ok(transformed_response(bytes, &output_mime, &etag, max_age));
            }
            Ok(None) => {}
            Err(err) => tracing::warn!(error = %err, "transform cache read failed"),
        }
    }

    crate::app::require_plugin(TRANSFORM_PLUGIN)
        .await
        .map_err(|message| api_error(message).with_status_code(StatusCode::BAD_REQUEST))?;

    let input = tokio::fs::read(file_path)
        .await
        .map_err(|err| api_error(format!("failed to read file: {err}")))?;

    let output = run_media_plugin(TRANSFORM_PLUGIN, input, config)
        .await
        .map_err(|err| {
            api_error(format!("image transformation failed: {err}"))
                .with_status_code(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    if let Some(broker) = cache
        && let Err(err) = broker.set_transform_cache(&etag, &output, ttl).await
    {
        tracing::warn!(error = %err, "transform cache write failed");
    }

    Ok(transformed_response(output, &output_mime, &etag, max_age))
}

/// Generate a signed download token for a file in a private bucket.
///
/// Validates that the bucket is private, the file path falls within the
/// bucket, and the file exists on disk before issuing the token.
/// Requires client authentication via the API-key header.
#[axum::debug_handler]
pub(super) async fn sign_download(
    State(state): State<AppState>,
    client: ClientExtractor,
    Json(config): Json<SignDownloadRequest>,
) -> ApiResponse<String> {
    config
        .validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    // Fail fast on transformation problems before touching the database.
    if let Some(transformation) = &config.image_transformation {
        transformation
            .validate_operations()
            .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

        let source_mime = mime_guess::from_path(&config.path)
            .first_or_octet_stream()
            .to_string();
        if !source_mime.starts_with("image/") {
            return Err(api_error(format!(
                "image_transformation requires an image/* file, got '{source_mime}'"
            ))
            .with_status_code(StatusCode::BAD_REQUEST));
        }

        if let Err(message) = crate::app::require_plugin(TRANSFORM_PLUGIN).await {
            return Err(api_error(message).with_status_code(StatusCode::BAD_REQUEST));
        }
    }

    let bucket_data = bucket::get(&config.bucket, state.db()).await?;

    if bucket_data.public {
        return Err(
            api_error("signed downloads are only available for private buckets")
                .with_status_code(StatusCode::BAD_REQUEST),
        );
    }

    // Validate the file path is within the bucket
    let cleaned_path = config.path.trim_start_matches('/');
    if cleaned_path.is_empty() {
        return Err(api_error("path must not be empty").with_status_code(StatusCode::BAD_REQUEST));
    }

    for component in std::path::Path::new(cleaned_path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(
                api_error("path contains invalid components: '..' is not allowed")
                    .with_status_code(StatusCode::BAD_REQUEST),
            );
        }
    }

    let bucket_path = bucket_data.path.trim_end_matches('/');
    let expected_prefix = format!("{bucket_path}/");
    let full_path = format!("{bucket_path}/{cleaned_path}");

    if !full_path.starts_with(&expected_prefix) && full_path != bucket_path {
        return Err(api_error("path is not within the specified bucket")
            .with_status_code(StatusCode::FORBIDDEN));
    }

    // Validate the file exists on disk
    let root_dir = state.config().root_dir()?;
    let file_path = root_dir.join(full_path.trim_start_matches('/'));
    let is_file = tokio::fs::metadata(&file_path)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false);
    if !is_file {
        return Err(api_error("file not found").with_status_code(StatusCode::NOT_FOUND));
    }

    // Resolve the requesting client's asset_owner ID
    let owner_id = asset_owner_id(AssetOwnerName::Client, client.id(), state.db()).await?;

    // Check if client is the bucket owner
    let is_bucket_owner = bucket_data.owner_id == owner_id;

    if !is_bucket_owner {
        // Check file-level read permission
        let asset = asset::get_by_bucket_and_path(state.db(), bucket_data.id, cleaned_path).await?;
        match asset {
            Some(asset) => {
                let has_perm =
                    asset::has_permission(state.db(), asset.id, owner_id, PermissionLevel::Read)
                        .await?;
                if !has_perm {
                    return Err(api_error("access denied for the specified file")
                        .with_status_code(StatusCode::FORBIDDEN));
                }
            }
            None => {
                return Err(api_error("file not found").with_status_code(StatusCode::NOT_FOUND));
            }
        }
    }

    let exp = seconds_from_now(config.expires)?;
    let (pid, key) = client::get_claims_data(state.db(), &client.id(), state.secrets()).await?;

    let info = DownloadInfo {
        client_id: pid,
        path: config.path,
        bucket_pid: config.bucket,
        exp,
        image_transformation: config.image_transformation,
        client_key: Some(key.clone()),
    };

    let token = info.sign(&key, state.hasher())?;
    api_response(token)
}

/// Serve a file from a private bucket using a signed download token.
///
/// Supports `Range` headers for partial-content (HTTP 206) responses.
pub(super) async fn serve_download(
    State(state): State<AppState>,
    headers: HeaderMap,
    DownloadMiddleware(info): DownloadMiddleware,
) -> Result<Response, ResponseError> {
    let file_path = resolve_file_path(&state, &info.bucket_pid, &info.path).await?;

    let metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(|_| api_error("file not found").with_status_code(StatusCode::NOT_FOUND))?;
    let file_size = metadata.len();

    let mime = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Tokens can carry a transformation; serve the rendition instead of the
    // raw file (Range does not apply to a re-encoded byte stream).
    if let Some(ref transformation) = info.image_transformation {
        let max_age_ceiling = (info.exp - seconds_from_now(0)?).max(0) as u64;
        return serve_transformed(
            &state,
            &headers,
            &file_path,
            file_size,
            mtime_nanos(&metadata),
            transformation,
            Some(max_age_ceiling),
        )
        .await;
    }

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    // Handle Range request
    if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = range_header.to_str().map_err(|_| {
            api_error("invalid Range header").with_status_code(StatusCode::RANGE_NOT_SATISFIABLE)
        })?;

        if let Some((start, end)) = parse_range(range_str, file_size) {
            let content_length = end - start + 1;

            if let Ok(val) = HeaderValue::from_str(&content_length.to_string()) {
                response_headers.insert(header::CONTENT_LENGTH, val);
            }
            if let Ok(val) = HeaderValue::from_str(&format!("bytes {start}-{end}/{file_size}")) {
                response_headers.insert(header::CONTENT_RANGE, val);
            }

            let mut file = tokio::fs::File::open(&file_path)
                .await
                .map_err(|e| api_error(format!("failed to open file: {e}")))?;
            file.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| api_error(format!("failed to seek file: {e}")))?;

            let limited = file.take(content_length);
            let stream = ReaderStream::new(limited);
            let body = Body::from_stream(stream);

            return Ok((StatusCode::PARTIAL_CONTENT, response_headers, body).into_response());
        }

        return Err(
            api_error("Range not satisfiable").with_status_code(StatusCode::RANGE_NOT_SATISFIABLE)
        );
    }

    // Full file response
    if let Ok(val) = HeaderValue::from_str(&file_size.to_string()) {
        response_headers.insert(header::CONTENT_LENGTH, val);
    }

    let file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|e| api_error(format!("failed to open file: {e}")))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok((response_headers, body).into_response())
}

/// Entry point for `GET <mount>/<path>?image_transformation=<url-encoded JSON>`
/// on public buckets and static folders.
///
/// Called by the transform-aware mount wrapper in `app.rs` for GET requests
/// carrying the parameter; all other traffic passes through to `ServeDir`.
pub(crate) async fn serve_direct_transformed(
    state: AppState,
    base: PathBuf,
    request: axum::extract::Request,
) -> Response {
    match direct_transform(state, base, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn direct_transform(
    state: AppState,
    base: PathBuf,
    request: axum::extract::Request,
) -> Result<Response, ResponseError> {
    // The transformation config arrives as URL-encoded JSON.
    let params = Query::<HashMap<String, String>>::try_from_uri(request.uri()).map_err(|err| {
        api_error(format!("invalid query string: {err}")).with_status_code(StatusCode::BAD_REQUEST)
    })?;
    let raw = params.get("image_transformation").ok_or_else(|| {
        api_error("image_transformation query parameter is missing")
            .with_status_code(StatusCode::BAD_REQUEST)
    })?;
    let config: ImageTransformationConfig = serde_json::from_str(raw).map_err(|err| {
        api_error(format!("invalid image_transformation: {err}"))
            .with_status_code(StatusCode::BAD_REQUEST)
    })?;

    // Unauthenticated surface: typed operations only.
    if config.custom_filters.is_some() {
        return Err(
            api_error("custom_filters is not allowed for direct downloads")
                .with_status_code(StatusCode::BAD_REQUEST),
        );
    }
    config
        .validate_operations()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    // Resolve the file under the mount's base directory.
    let relative = request.uri().path();
    for component in Path::new(relative).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(
                api_error("path contains invalid components: '..' is not allowed")
                    .with_status_code(StatusCode::BAD_REQUEST),
            );
        }
    }

    let joined = base.join(relative.trim_start_matches('/'));
    let (canonical, metadata) = tokio::task::spawn_blocking(move || {
        let canonical = std::fs::canonicalize(&joined)?;
        let metadata = std::fs::metadata(&canonical)?;
        anyhow::Ok((canonical, metadata))
    })
    .await
    .map_err(|err| api_error(format!("failed to resolve path: {err}")))?
    .map_err(|_| api_error("file not found").with_status_code(StatusCode::NOT_FOUND))?;

    let canonical_base = tokio::fs::canonicalize(&base)
        .await
        .map_err(|_| api_error("file not found").with_status_code(StatusCode::NOT_FOUND))?;
    if !canonical.starts_with(&canonical_base) {
        return Err(
            api_error("path escapes root directory").with_status_code(StatusCode::BAD_REQUEST)
        );
    }

    if !metadata.is_file() {
        return Err(api_error("file not found").with_status_code(StatusCode::NOT_FOUND));
    }

    // Only images can be transformed.
    let source_mime = mime_guess::from_path(&canonical)
        .first_or_octet_stream()
        .to_string();
    if !source_mime.starts_with("image/") {
        return Err(api_error(format!(
            "image_transformation requires an image/* file, got '{source_mime}'"
        ))
        .with_status_code(StatusCode::BAD_REQUEST));
    }

    let size = metadata.len();
    let mtime = mtime_nanos(&metadata);

    serve_transformed(
        &state,
        request.headers(),
        &canonical,
        size,
        mtime,
        &config,
        None,
    )
    .await
}

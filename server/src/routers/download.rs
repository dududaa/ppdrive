//! Download handlers for private bucket file access.
//!
//! `POST /download/sign` generates a time-limited signed token for a file.
//! `GET /download/{token}` serves the file with Range-header support.

use crate::routers::middlewares::{ClientExtractor, DownloadMiddleware};
use crate::routers::resp::{api_error, api_response, ApiResponse, ResponseError};
use crate::state::AppState;
use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use shared::AssetOwnerName;
use shared::db::{asset, bucket, client};
use shared::asset_owner_id;
use shared::db::asset::models::PermissionLevel;
use shared::server::{DownloadInfo, SignDownloadRequest};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use validator::Validate;
use shared::seconds_from_now;

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
    let bucket_root = root_dir.join(&bucket.path);

    for component in std::path::Path::new(relative_path).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(api_error("path contains invalid components: '..' is not allowed")
                .with_status_code(StatusCode::BAD_REQUEST));
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
        return Err(api_error("file not found")
            .with_status_code(StatusCode::NOT_FOUND));
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

    let bucket_data = bucket::get(&config.bucket, state.db()).await?;

    if bucket_data.public {
        return Err(api_error("signed downloads are only available for private buckets")
            .with_status_code(StatusCode::BAD_REQUEST));
    }

    // Validate the file path is within the bucket
    let cleaned_path = config.path.trim_start_matches('/');
    if cleaned_path.is_empty() {
        return Err(api_error("path must not be empty")
            .with_status_code(StatusCode::BAD_REQUEST));
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
    let file_path = root_dir.join(&full_path);
    let is_file = tokio::fs::metadata(&file_path).await.map(|m| m.is_file()).unwrap_or(false);
    if !is_file {
        return Err(api_error("file not found")
            .with_status_code(StatusCode::NOT_FOUND));
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
                let has_perm = asset::has_permission(
                    state.db(),
                    asset.id,
                    owner_id,
                    PermissionLevel::Read,
                ).await?;
                if !has_perm {
                    return Err(api_error("access denied for the specified file")
                        .with_status_code(StatusCode::FORBIDDEN));
                }
            }
            None => {
                return Err(api_error("file not found")
                    .with_status_code(StatusCode::NOT_FOUND));
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

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime).unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response_headers.insert(
        header::ACCEPT_RANGES,
        HeaderValue::from_static("bytes"),
    );

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

            let mut file = tokio::fs::File::open(&file_path).await
                .map_err(|e| api_error(format!("failed to open file: {e}")))?;
            file.seek(std::io::SeekFrom::Start(start)).await
                .map_err(|e| api_error(format!("failed to seek file: {e}")))?;

            let limited = file.take(content_length);
            let stream = ReaderStream::new(limited);
            let body = Body::from_stream(stream);

            return Ok((StatusCode::PARTIAL_CONTENT, response_headers, body).into_response());
        }

        return Err(api_error("Range not satisfiable")
            .with_status_code(StatusCode::RANGE_NOT_SATISFIABLE));
    }

    // Full file response
    if let Ok(val) = HeaderValue::from_str(&file_size.to_string()) {
        response_headers.insert(header::CONTENT_LENGTH, val);
    }

    let file = tokio::fs::File::open(&file_path).await
        .map_err(|e| api_error(format!("failed to open file: {e}")))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok((response_headers, body).into_response())
}

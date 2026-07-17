use crate::routers::DEFAULT_BODY_LIMIT;
use crate::routers::middlewares::{ClientExtractor, UploadMiddleware};
use crate::routers::resp::{ApiResponse, api_error, api_response};
use crate::state::AppState;
use anyhow::anyhow;
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use shared::server::*;
use shared::{client, generate_nano_id, root_dir};
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use validator::Validate;

/// Creates an upload session and returns the session token. If [AppConfig::use_session] is enabled,
/// we create the session id.
#[axum::debug_handler]
pub(super) async fn create_session(
    State(state): State<AppState>,
    client: ClientExtractor,
    Json(config): Json<UploadUrlConfig>,
) -> ApiResponse<String> {
    config
        .validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let resumable = config.resumable.unwrap_or_default();
    if resumable && state.config().message_broker.is_none() {
        return Err(
            api_error("resumable upload is impossible without a message broker.")
                .with_status_code(StatusCode::BAD_REQUEST),
        );
    }

    let mut session_id = None;
    if let AssetType::File = config.asset_type {
        let size = config
            .target_filesize
            .ok_or(api_error("target_filesize is required for file upload"))?;

        if size >= DEFAULT_BODY_LIMIT as u64 && !config.resumable.unwrap_or_default() {
            return Err(api_error(format!(
                "Files larger than ${DEFAULT_BODY_LIMIT} must be resumable."
            ))
            .with_status_code(StatusCode::PAYLOAD_TOO_LARGE));
        }

        // SessionID is tightly coupled with MessageBroker. No need for a session if broker is not provided.
        if resumable && state.config().message_broker.is_some() {
            session_id = Some(generate_nano_id(24));
        }
    }

    let exp = seconds_from_now(config.expires)?;
    let (key, pid) = client::get_claims_data(state.db(), &client.id()).await?;

    let data = UploadInfo {
        client_id: pid,
        session_id,
        chunk_session_expiration: config.expires,
        config: Some(config),
        chunk_index: 0,
        exp,
    };

    let token = data.sign(&key)?;
    api_response(token)
}

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

    let config = info.config.clone();
    let config = config.ok_or(api_error("missing configuration"))?;
    let root_dir = state.config().root_dir()?;
    let target_path = root_dir.join(&config.path);

    let parent_dir = target_path.parent().unwrap_or(&root_dir);
    if target_path.exists() && !config.overwrite.unwrap_or_default() {
        return Err(api_error("Asset already exists"));
    }

    if parent_dir != root_dir && !parent_dir.exists() && !config.create_parents.unwrap_or_default()
    {
        return Err(api_error("Parent directory does not exist"));
    }

    match config.asset_type {
        AssetType::File => handle_session(&state, info, body).await,

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

async fn handle_session(
    state: &AppState,
    info: UploadInfo,
    body: Bytes,
) -> ApiResponse<Option<String>> {
    let session_id = info.session_id.clone();
    match get_next_session(state, info, body).await {
        Ok(token) => api_response(token),
        Err(err) => {
            if let Some(id) = session_id {
                let tmp_path = root_dir()?.join("tmp").join(id);
                if let Err(err) = tokio::fs::remove_file(tmp_path).await {
                    tracing::error!("unable to clean up file after failure: {err}");
                }
            }

            Err(api_error(err.to_string()))
        }
    }
}

/// Upload file and get next session token.
async fn get_next_session(
    state: &AppState,
    info: UploadInfo,
    body: Bytes,
) -> anyhow::Result<Option<String>> {
    let tmp_dir = root_dir()?.join("tmp");
    let root_dir = state.config().root_dir()?;

    if !tmp_dir.exists() {
        tokio::fs::create_dir(&tmp_dir).await?;
    }

    let config = info
        .config
        .clone()
        .ok_or(anyhow!("missing configuration"))?;

    let session_id = info.session_id.clone();
    let target_path = root_dir.join(config.path.trim_start_matches("/"));
    let parent_dir = target_path.parent().unwrap_or(&root_dir);

    let target_filesize = config.target_filesize.ok_or(anyhow!(
        "Unable to determine target filesize. Please specify \"target_filesize\" in upload options."
    ))?;

    let resumable = config.resumable.unwrap_or_default();
    let mut next_token = None;

    let (tmp_path, completed) =
        upload_file(session_id.clone(), &tmp_dir, &body, target_filesize).await?;

    if !completed && resumable {
        let session_id = session_id.clone().ok_or(anyhow!("session_id not found"))?;
        let key = client::get_key(state.db(), &info.client_id).await?;
        let mut info = info.clone();

        let broker = state.broker()?;
        broker.upsert_upload_info(&session_id, &info).await?;

        let token = info.resign(&key)?;
        next_token = Some(token);
    }

    if completed {
        if parent_dir != root_dir && !parent_dir.exists() {
            tokio::fs::create_dir_all(&parent_dir).await?;
        }

        tokio::fs::rename(tmp_path, target_path).await?;
        if let Some(id) = session_id {
            let broker = state.broker()?;
            broker.remove_upload_info(&id).await?;
        }
    }

    Ok(next_token)
}

async fn upload_file(
    session_id: Option<String>,
    tmp_dir: &Path,
    data: &Bytes,
    target_filesize: u64,
) -> anyhow::Result<(PathBuf, bool)> {
    let id = session_id.ok_or(anyhow!("Unable to find session id"))?;

    let tmp_path = tmp_dir.join(&id);
    let mut tmp_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp_path)
        .await?;

    let filesize = tmp_file.metadata().await?.len();
    let writing_size = data.len();
    tracing::debug!("filesize {filesize}, writing_size {writing_size}");

    tmp_file.write_all(data).await?;
    tmp_file.flush().await?;

    let tmp_size = tmp_file.metadata().await?.len();
    let completed = tmp_size >= target_filesize;
    Ok((tmp_path, completed))
}

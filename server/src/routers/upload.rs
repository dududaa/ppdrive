use crate::routers::middlewares::{ClaimsExtractor, ClientExtractor};
use crate::routers::payloads::{AssetType, UploadUrlConfig};
use crate::routers::resp::{ApiResponse, api_error, api_response};
use crate::routers::session::{check_session, next_session_token};
use crate::routers::{DEFAULT_BODY_LIMIT, session};
use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use anyhow::anyhow;
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use shared::root_dir;
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

        let id = session::create_session(&state).await?;
        session_id = Some(id);
    }

    let exp = config.expires;
    let data = ClaimsData::Upload {
        session_id,
        session_resume: false,
        config,
    };
    
    let claims = Claims::new(client.id(), exp, data)?;

    let token = create_jwt(state.secrets(), &claims)?;
    api_response(token)
}

#[axum::debug_handler]
pub(super) async fn play_session(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    body: Bytes,
) -> ApiResponse<Option<String>> {
    match claims.data() {
        ClaimsData::Upload {
            config,
            session_id,
            session_resume,
        } => {
            if *session_resume {
                return Err(api_error("Unsupported session token.")
                    .with_status_code(StatusCode::UNAUTHORIZED));
            }

            let use_session = state.config().use_session;
            if use_session {
                let session_id = session_id.clone().ok_or(api_error("Missing session id"))?;
                if !check_session(&state, &session_id).await? {
                    return Err(api_error(
                        "Session id is already used. Please create a new session id.",
                    ));
                }
            }

            let root_dir = state.config().root_dir()?;
            let target_path = root_dir.join(&config.path);

            let parent_dir = target_path.parent().unwrap_or(&root_dir);
            if target_path.exists() && !config.overwrite.unwrap_or_default() {
                return Err(api_error("Asset already exists"));
            }

            if parent_dir != root_dir
                && !parent_dir.exists()
                && !config.create_parents.unwrap_or_default()
            {
                return Err(api_error("Parent directory does not exist"));
            }

            match config.asset_type {
                AssetType::File => {
                    return handle_session(session_id, config, &state, &claims, body).await;
                }

                AssetType::Folder => {
                    if config.create_parents.unwrap_or_default() {
                        tokio::fs::create_dir_all(target_path).await?;
                    } else {
                        tokio::fs::create_dir(target_path).await?;
                    }
                }
            }

            if use_session {
                let session_id = session_id.clone().ok_or(api_error("Missing session id."))?;
                session::revoke_token(&state, &session_id).await?;
            }

            api_response(None)
        }
    }
}

#[axum::debug_handler]
pub(super) async fn play_next_session(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    body: Bytes,
) -> ApiResponse<Option<String>> {
    match &claims.data() {
        ClaimsData::Upload {
            config,
            session_id,
            session_resume,
        } => {
            if !*session_resume {
                return Err(api_error("Unsupported session token.")
                    .with_status_code(StatusCode::UNAUTHORIZED));
            }

            let root_dir = state.config().root_dir()?;
            let target_path = root_dir.join(&config.path);

            if target_path.exists() && !config.overwrite.unwrap_or_default() {
                return Err(api_error("Asset already exists"));
            }

            let next_token = match config.asset_type {
                AssetType::File => handle_session(session_id, config, &state, &claims, body).await,
                AssetType::Folder => Err(api_error("Unsupported upload operation")),
            }?;

            let use_session = state.config().use_session;
            if next_token.data().is_none() && use_session {
                let session_id = session_id.clone().ok_or(api_error("Missing session id."))?;
                session::revoke_token(&state, &session_id).await?;
            }

            Ok(next_token)
        }
    }
}

async fn handle_session(
    session_id: &Option<String>,
    config: &UploadUrlConfig,
    state: &AppState,
    claims: &ClaimsExtractor,
    body: Bytes,
) -> ApiResponse<Option<String>> {
    match get_next_session(state, claims, body, session_id, config).await {
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
    extractor: &ClaimsExtractor,
    body: Bytes,
    session_id: &Option<String>,
    config: &UploadUrlConfig,
) -> anyhow::Result<Option<String>> {
    let tmp_dir = root_dir()?.join("tmp");
    let root_dir = state.config().root_dir()?;

    if !tmp_dir.exists() {
        tokio::fs::create_dir(&tmp_dir).await?;
    }

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
        let token = next_session_token(state, *extractor.sub(), extractor.data().clone())?;
        next_token = Some(token);
    }

    if completed {
        if parent_dir != root_dir && !parent_dir.exists() {
            tokio::fs::create_dir_all(&parent_dir).await?;
        }

        tokio::fs::rename(tmp_path, target_path).await?;
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

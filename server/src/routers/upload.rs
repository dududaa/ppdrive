use crate::routers::middlewares::{ClaimsExtractor, ClientExtractor};
use crate::routers::payloads::{AssetType, UploadUrlConfig};
use crate::routers::resp::{ApiResponse, api_error, api_response};
use crate::routers::session;
use crate::routers::session::{check_session, next_session_token};
use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use anyhow::anyhow;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use shared::{generate_nano_id, root_dir};
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

    let use_session = state.config().use_session;
    let mut session_id = None;
    if use_session {
        let id = session::create_session_id(&state).await?;
        session_id = Some(id);
    }

    let exp = config.expires;
    let data = ClaimsData::Upload { session_id, config };
    let claims = Claims::new(client.id(), exp, data)?;

    let token = create_jwt(state.secrets(), &claims)?;
    api_response(token)
}

#[axum::debug_handler]
pub(super) async fn play_session(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    multipart: Multipart,
) -> ApiResponse<Option<String>> {
    match claims.data() {
        ClaimsData::Upload { config, session_id } => {
            let use_session = state.config().use_session;
            if use_session {
                let session_id = session_id.clone().ok_or(api_error("missing session id"))?;
                if !check_session(&state, &session_id).await? {
                    return Err(api_error(
                        "session id is already used. Please create a new session id.",
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
                    return handle_session(session_id, config, &state, &claims, multipart).await;
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
                let session_id = session_id.clone().ok_or(api_error("missing session id"))?;
                session::revoke_token(&state, &session_id).await?;
            }

            api_response(None)
        }
    }
}

pub(super) async fn play_next_session(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    multipart: Multipart,
) -> ApiResponse<Option<String>> {
    match &claims.data() {
        ClaimsData::Upload { config, session_id } => {
            let root_dir = state.config().root_dir()?;
            let target_path = root_dir.join(&config.path);

            if target_path.exists() && !config.overwrite.unwrap_or_default() {
                return Err(api_error("Asset already exists"));
            }

            match config.asset_type {
                AssetType::File => {
                    handle_session(session_id, config, &state, &claims, multipart).await
                }
                AssetType::Folder => Err(api_error("Unsupported upload operation")),
            }
        }
    }
}

async fn handle_session(
    session_id: &Option<String>,
    config: &UploadUrlConfig,
    state: &AppState,
    claims: &ClaimsExtractor,
    multipart: Multipart,
) -> ApiResponse<Option<String>> {
    match get_next_session(state, claims, multipart, session_id, config).await {
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
    mut multipart: Multipart,
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

    while let Ok(Some(field)) = multipart.next_field().await {
        let target_path = target_path.clone();
        let name = field
            .name()
            .map(|name| name.to_string())
            .unwrap_or_default();

        if name.as_str() == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|err| anyhow!("unable to extract raw file {err}"))?;

            let (tmp_path, completed) =
                upload_file(session_id.clone(), &tmp_dir, data, target_filesize).await?;

            if !completed && resumable {
                let token = next_session_token(state, *extractor.sub(), extractor.data().clone())?;
                next_token = Some(token);
            }

            if parent_dir != root_dir && !parent_dir.exists() {
                tokio::fs::create_dir_all(&parent_dir).await?;
            }

            tokio::fs::rename(tmp_path, target_path).await?;
        }
    }

    Ok(next_token)
}

async fn upload_file(
    session_id: Option<String>,
    tmp_dir: &Path,
    data: Bytes,
    target_filesize: u64,
) -> anyhow::Result<(PathBuf, bool)> {
    let id = session_id.unwrap_or(generate_nano_id(24));

    let tmp_path = tmp_dir.join(&id);
    let mut tmp_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp_path)
        .await?;

    tmp_file.write_all(&data).await?;
    tmp_file.flush().await?;

    let tmp_size = tmp_file.metadata().await?.len();
    let completed = tmp_size >= target_filesize;
    Ok((tmp_path, completed))
}

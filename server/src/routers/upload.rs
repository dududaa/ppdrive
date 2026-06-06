use crate::routers::middlewares::{ClaimsExtractor, ClientExtractor};
use crate::routers::payloads::{AssetType, UploadUrlConfig};
use crate::routers::resp::{ApiResponse, api_error, api_response};
use crate::routers::session;
use crate::routers::session::{check_session, next_token};
use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use anyhow::anyhow;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use shared::generate_nano_id;
use std::path::PathBuf;
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
    let claims = Claims {
        sub: client.id(),
        exp,
        data,
    };

    let token = create_jwt(state.secrets(), &claims)?;
    api_response(token)
}

#[axum::debug_handler]
pub(super) async fn play_session(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    mut multipart: Multipart,
) -> ApiResponse<Option<String>> {
    match &claims.data {
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
            let path = root_dir.join(&config.path);

            let parent_dir = path.parent().unwrap_or(&root_dir);
            if path.exists() && !config.overwrite.unwrap_or_default() {
                return Err(api_error("Asset already exists"));
            }

            if parent_dir != &root_dir
                && !parent_dir.exists()
                && !config.create_parents.unwrap_or_default()
            {
                return Err(api_error("Parent directory does not exist"));
            }

            match config.asset_type {
                AssetType::File => {
                    let resumable = config.resumable.unwrap_or_default();
                    let filesize = config
                        .filesize
                        .ok_or(api_error("unable to determine target filesize"))?;

                    let tmp_dir = state.config().root_dir()?.join("tmp");
                    tokio::fs::create_dir(&tmp_dir).await?;

                    while let Ok(Some(field)) = multipart.next_field().await {
                        let name = field
                            .name()
                            .map(|name| name.to_string())
                            .unwrap_or_default();

                        if name.as_str() == "file" {
                            let data = field.bytes().await.map_err(|err| api_error(err))?;
                            let (tmp_path, completed) =
                                upload_file(session_id.clone(), &tmp_dir, data, filesize).await?;

                            if !completed && resumable {
                                let next_token =
                                    next_token(&state, claims.sub, claims.data.clone())?;
                                return api_response(Some(next_token));
                            }

                            if parent_dir != root_dir && !parent_dir.exists() {
                                tokio::fs::create_dir_all(&parent_dir).await?;
                            }

                            tokio::fs::rename(tmp_path, &path).await?;
                        }
                    }
                }

                AssetType::Folder => {
                    if config.create_parents.unwrap_or_default() {
                        tokio::fs::create_dir_all(path).await?;
                    } else {
                        tokio::fs::create_dir(path).await?;
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

async fn upload_file(
    session_id: Option<String>,
    tmp_dir: &PathBuf,
    data: Bytes,
    target_filesize: u64,
) -> anyhow::Result<(PathBuf, bool)> {
    let id = session_id.unwrap_or(generate_nano_id(24));

    let tmp_path = tmp_dir.join(&id);
    let mut tmp_file = OpenOptions::new()
        .read(true)
        .append(true)
        .open(&tmp_path)
        .await?;

    let tmp_size = tmp_file.metadata().await?.len();
    if (tmp_size + data.len() as u64) > target_filesize {
        tokio::fs::remove_file(&tmp_path).await?;
        return Err(anyhow!("Upload already completed for the id: {id}"));
    }

    tmp_file.write(&data).await?;
    let tmp_size = tmp_file.metadata().await?.len();
    let completed = tmp_size >= target_filesize;
    Ok((tmp_path, completed))
}

#[cfg(test)]
mod tests {
    use crate::app::create_app;
    use crate::routers::payloads::{AssetType, UploadUrlConfig, UploadUrlMethod};
    use crate::state::AppState;
    use axum_test::TestServer;
    use shared::client::create_client;

    #[tokio::test]
    async fn test_create_signed_url() -> anyhow::Result<()> {
        let (app, _) = create_app().await?;
        let state = AppState::new().await?;

        let client_header_key = state.config().client_header_key.clone();
        let client = create_client(state.pool(), state.secrets(), "Test Client", None).await?;

        let server = TestServer::new(app);
        let config = UploadUrlConfig {
            method: UploadUrlMethod::Post,
            asset_type: AssetType::File,
            path: "demo-file.png".to_string(),
            expires: 30,
            ..Default::default()
        };

        let base_request = || {
            server
                .post("/upload/signed")
                .json(&config)
                .content_type("application/json")
        };

        let mut resp = base_request().await;
        resp.assert_status_unauthorized();

        resp = base_request()
            .add_header(client_header_key, client.token())
            .await;
        resp.assert_status_ok();

        Ok(())
    }
}

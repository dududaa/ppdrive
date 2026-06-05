use crate::middlewares::{ClaimsExtractor, ClientExtractor};
use crate::payloads::{AssetType, UploadUrlConfig};
use crate::resp::{ApiResponse, api_error, api_response};
use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use axum_macros::debug_handler;
use shared::generate_nano_id;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use validator::Validate;

#[debug_handler]
async fn create_signed_url(
    State(state): State<AppState>,
    client: ClientExtractor,
    Json(config): Json<UploadUrlConfig>,
) -> ApiResponse<String> {
    config
        .validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let exp = config.expires;
    let data = ClaimsData::Upload {
        id: generate_nano_id(24),
        config,
    };

    let claims = Claims {
        sub: client.id(),
        exp,
        data,
    };

    let token = create_jwt(state.secrets(), &claims)?;
    api_response(token)
}

#[debug_handler]
async fn upload_asset(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    mut multipart: Multipart,
) -> ApiResponse<()> {
    match &claims.data {
        ClaimsData::Upload { id, config } => {
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
                    let filesize = config
                        .filesize
                        .ok_or(api_error("unable to determine target filesize"))?;

                    let resumable = config.resumable.unwrap_or_default();
                    let tmp_dir = state.config().root_dir()?.join("tmp");
                    tokio::fs::create_dir(&tmp_dir).await?;

                    while let Ok(Some(field)) = multipart.next_field().await {
                        let name = field
                            .name()
                            .map(|name| name.to_string())
                            .unwrap_or_default();

                        if name.as_str() == "file" {
                            let data = field.bytes().await.map_err(|err| api_error(err))?;
                            if resumable {
                                let tmp_path = tmp_dir.join(id);
                                let mut tmp_file = OpenOptions::new()
                                    .read(true)
                                    .append(true)
                                    .open(&tmp_path)
                                    .await?;

                                let tmp_size = tmp_file.metadata().await?.len();
                                if (tmp_size + data.len() as u64) > filesize {
                                    tokio::fs::remove_file(&tmp_path).await?;
                                    return Err(api_error(format!(
                                        "Upload already completed for the id: {id}"
                                    )));
                                }

                                tmp_file.write(&data).await?;
                                let tmp_size = tmp_file.metadata().await?.len();
                                if tmp_size >= filesize {
                                    if parent_dir != &root_dir && !parent_dir.exists() {
                                        tokio::fs::create_dir_all(&parent_dir).await?;
                                    }

                                    tokio::fs::rename(&tmp_path, &path).await?;
                                }
                            }
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
        }
    }

    // TODO: Invalidate token
    api_response(())
}

pub fn upload_routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_signed_url))
        .route("/session/play", post(upload_asset))
}

#[cfg(test)]
mod tests {
    use crate::app::create_app;
    use crate::payloads::{AssetType, UploadUrlConfig, UploadUrlMethod};
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

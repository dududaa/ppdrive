mod common;

use crate::common::{TestServerWrapper, upload_config};
use axum_test::multipart::{MultipartForm, Part};
use server::state::AppState;
use shared::client::create_client;
use shared::root_dir;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn test_create_upload_session() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let client_header_key = state.config().client_header_key.clone();
    let client = create_client(state.pool(), state.secrets(), "Test Client", None).await?;

    let server = TestServerWrapper::new().await?;
    let request = server.post("/upload/session", &upload_config());

    let mut resp = request.await;
    resp.assert_status_unauthorized();

    let request = server.post("/upload/session", &upload_config());
    resp = request.add_header(client_header_key, client.token()).await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
async fn test_play_upload_session() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let client_header_key = state.config().client_header_key.clone();
    let client = create_client(state.pool(), state.secrets(), "Test Client", None).await?;

    let server = TestServerWrapper::new().await?;
    let mut data = vec![];
    let filepath = root_dir()?.join("test-assets/demo.jpg");
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut data)
        .await?;

    let mut upload_config = upload_config();
    upload_config.create_parents = Some(true);
    upload_config.target_filesize = Some(data.len() as u64);

    let base_request = server.post("/upload/session", &upload_config);
    let token: String = base_request
        .add_header(client_header_key, client.token())
        .await
        .json();

    let form = MultipartForm::new().add_part("file", Part::bytes(data));
    let request = server
        .multipart("/upload/session/play", form)
        .authorization_bearer(token);

    let resp = request.await;
    resp.assert_status_ok();
    Ok(())
}

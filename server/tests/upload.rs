use axum::body::Bytes;
use futures_util::StreamExt;
mod common;

use crate::common::{TestServerWrapper, upload_config};
use server::state::AppState;
use shared::client::create_client;
use shared::root_dir;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

#[tokio::test]
async fn test_create_upload_session() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let client_header_key = state.config().client_header_key.clone();
    let client = create_client(state.db(), state.secrets(), "Test Client", None).await?;

    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let filemeta = tokio::fs::metadata(&filepath).await?;

    let mut upload_config = (&upload_config()).clone();
    upload_config.target_filesize = Some(filemeta.len());

    let url = "/upload/session";
    let server = TestServerWrapper::new().await?;
    let request = server.post(url, &upload_config);

    let mut resp = request.await;
    resp.assert_status_unauthorized();


    let request = server.post(url, &upload_config);
    resp = request.add_header(client_header_key, client.token()).await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
async fn test_play_upload_session() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let client_header_key = state.config().client_header_key.clone();
    let client = create_client(state.db(), state.secrets(), "Test Client", None).await?;

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
    upload_config.overwrite = Some(true);

    let token_url = "/upload/session";
    let upload_url = "/upload/session/play";

    // Unauthorized
    let request = server.post_bytes(upload_url, Bytes::copy_from_slice(&data));
    let resp = request.await;
    resp.assert_status_unauthorized();

    // Requesting token for a file upload without providing target_filesize resolves to 500
    let token_request = server.post(token_url, &upload_config);
    let request = token_request
        .add_header(&client_header_key, client.token());

    let resp = request.await;
    resp.assert_status_internal_server_error();

    // Success
    upload_config.target_filesize = Some(data.len() as u64);
    let token_request = server.post(token_url, &upload_config);
    let token: String = token_request
        .add_header(&client_header_key, client.token())
        .await
        .json();

    let request = server
        .post_bytes(upload_url, Bytes::copy_from_slice(&data))
        .authorization_bearer(&token);

    let resp = request.await;
    resp.assert_status_ok();

    // Overwrite fails
    upload_config.overwrite = Some(false);
    let token_request = server.post(token_url, &upload_config);
    let token: String = token_request
        .add_header(&client_header_key, client.token())
        .await
        .json();

    let request = server
        .post_bytes(upload_url, Bytes::copy_from_slice(&data))
        .authorization_bearer(&token);

    let resp = request.await;
    resp.assert_status_internal_server_error();

    // Overwrite succeed
    upload_config.overwrite = Some(true);
    let token_request = server.post(token_url, &upload_config);
    let token: String = token_request
        .add_header(&client_header_key, client.token())
        .await
        .json();

    let request = server
        .post_bytes(upload_url, Bytes::copy_from_slice(&data))
        .authorization_bearer(&token);

    let resp = request.await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
async fn test_play_upload_resumable_session() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let client_header_key = state.config().client_header_key.clone();
    let client = create_client(state.db(), state.secrets(), "Test Client", None).await?;

    let server = TestServerWrapper::new().await?;
    let mut upload_config = upload_config();

    let token_url = "/upload/session";
    let upload_url = "/upload/session/play";
    let play_next_url = "/upload/session/next";

    // Payload too large. You must set resumable
    let demo_filepath = root_dir()?.join("test-assets/resumable.png");
    let file = tokio::fs::File::open(demo_filepath).await?;
    let filesize = file.metadata().await?.len();

    upload_config.target_filesize = Some(filesize);
    let token_request = server.post(token_url, &upload_config);
    let resp = token_request
        .add_header(&client_header_key, client.token())
        .await;

    resp.assert_status_payload_too_large();

    // Successful token
    upload_config.create_parents = Some(true);
    upload_config.overwrite = Some(true);
    upload_config.resumable = Some(true);
    upload_config.path = "test-assets/uploads/resumable_output.png".to_string();

    let token_request = server.post(token_url, &upload_config);
    let resp = token_request
        .add_header(&client_header_key, client.token())
        .await;

    resp.assert_status_ok();

    // Upload the first chunk
    let chunk_size = 2 * 1024 * 1024;
    let mut stream = ReaderStream::with_capacity(file, chunk_size);
    let mut next_token: Option<String> = None;

    if let Some(Ok(first_chunk)) = stream.next().await {
        let token: String = resp.json();
        let request = server
            .post_bytes(upload_url, first_chunk)
            .authorization_bearer(token);

        let resp = request.await;
        resp.assert_status_ok();

        next_token = resp.json();
        assert!(next_token.is_some());
    }

    // Upload the remaining chunks chunk
    while let Some(Ok(next_chunk)) = stream.next().await
        && let Some(token) = &next_token
    {
        let request = server
            .patch_bytes(play_next_url, next_chunk)
            .authorization_bearer(token);

        let resp = request.await;
        resp.assert_status_ok();

        next_token = resp.json();
    }

    Ok(())
}

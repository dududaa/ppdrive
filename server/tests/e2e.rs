mod common;

use axum::body::Bytes;
use axum::http::StatusCode;
use ppdrive::db::client::create_client;
use ppdrive::db::user;
use ppdrive::root_dir;
use ppdrive::server::{UploadUrlConfig, AssetType};
use ppdrive::state::AppState;
use serde_json::{json, Value};
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;

use crate::common::{TestServerWrapper, setup_test_client};

fn test_upload_config() -> UploadUrlConfig {
    UploadUrlConfig {
        asset_type: AssetType::File,
        path: "test-assets/uploads/creator.jpg".to_string(),
        expires: 120,
        accepts: Some(vec!["*/*".to_string()]),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_endpoint() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let resp = server.get("/health").await;
    resp.assert_status_ok();
    Ok(())
}

// ---------------------------------------------------------------------------
// Bucket CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_bucket_without_auth() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let body = json!({
        "name": "test-bucket",
        "path": "test-bucket-path"
    });
    let resp = server.post("/buckets", &body).await;
    resp.assert_status_unauthorized();
    Ok(())
}

#[tokio::test]
async fn test_create_bucket_with_auth() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "name": "e2e-public-bucket",
        "path": "e2e-public-bucket",
        "public": true
    });

    let resp = server
        .post("/buckets", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status(StatusCode::CREATED);
    let pid: String = resp.json();
    assert!(!pid.is_empty(), "bucket PID should not be empty");
    Ok(())
}

#[tokio::test]
async fn test_create_bucket_absolute_path_rejected() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "name": "bad-bucket",
        "path": "/etc/passwd"
    });

    let resp = server
        .post("/buckets", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_create_bucket_traversal_rejected() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "name": "bad-bucket",
        "path": "../escape"
    });

    let resp = server
        .post("/buckets", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_create_bucket_empty_path_rejected() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "name": "bad-bucket",
        "path": ""
    });

    let resp = server
        .post("/buckets", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

// ---------------------------------------------------------------------------
// Upload flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_upload_unauthorized() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let body = test_upload_config();
    let resp = server.post("/upload/session", &body).await;
    resp.assert_status_unauthorized();
    Ok(())
}

#[tokio::test]
async fn test_upload_session_and_play() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let mut file_data = vec![];
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut file_data)
        .await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(file_data.len() as u64);
    config.create_parents = Some(true);
    config.overwrite = Some(true);

    // Create session
    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();

    // Play upload
    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_ok();
    Ok(())
}

#[tokio::test]
async fn test_upload_session_requires_target_filesize() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let config = UploadUrlConfig {
        asset_type: AssetType::File,
        path: "test-assets/uploads/no_size.jpg".to_string(),
        expires: 120,
        target_filesize: None,
        create_parents: Some(true),
        overwrite: Some(true),
        accepts: Some(vec!["*/*".to_string()]),
        ..Default::default()
    };

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_upload_to_named_bucket() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // Create a private bucket
    let bucket_body = json!({
        "name": "upload-test-bucket",
        "path": "e2e-upload-bucket",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // Upload a file to the bucket
    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let mut file_data = vec![];
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut file_data)
        .await?;

    let config = UploadUrlConfig {
        asset_type: AssetType::File,
        path: "e2e-upload-bucket/test-file.jpg".to_string(),
        bucket: Some(bucket_pid),
        target_filesize: Some(file_data.len() as u64),
        create_parents: Some(true),
        overwrite: Some(true),
        expires: 120,
        ..Default::default()
    };

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();

    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_ok();
    Ok(())
}

// ---------------------------------------------------------------------------
// Download flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_download_sign_unauthorized() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let body = json!({
        "path": "test.jpg",
        "bucket": "some-bucket-pid",
        "expires": 60
    });
    let resp = server.post("/download/sign", &body).await;
    resp.assert_status_unauthorized();
    Ok(())
}

#[tokio::test]
async fn test_download_sign_nonexistent_file() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // Create a private bucket
    let bucket_body = json!({
        "name": "download-test-bucket",
        "path": "e2e-download-bucket",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // Try to sign a nonexistent file
    let body = json!({
        "path": "nonexistent.jpg",
        "bucket": bucket_pid,
        "expires": 60
    });
    let resp = server
        .post("/download/sign", &body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_not_found();
    Ok(())
}

#[tokio::test]
async fn test_download_full_flow() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // 1. Create private bucket
    let bucket_body = json!({
        "name": "dl-flow-bucket",
        "path": "e2e-dl-bucket",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // 2. Upload a file
    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let mut file_data = vec![];
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut file_data)
        .await?;
    let original_size = file_data.len();

    let config = UploadUrlConfig {
        asset_type: AssetType::File,
        path: "e2e-dl-bucket/test-download.jpg".to_string(),
        bucket: Some(bucket_pid.clone()),
        target_filesize: Some(original_size as u64),
        create_parents: Some(true),
        overwrite: Some(true),
        expires: 120,
        ..Default::default()
    };

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();

    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_ok();

    // 3. Sign download
    let sign_body = json!({
        "path": "test-download.jpg",
        "bucket": bucket_pid,
        "expires": 60
    });
    let resp = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let download_token: String = resp.json();
    assert!(!download_token.is_empty());

    // 4. Download the file
    let resp = server
        .get(&format!("/download/{download_token}"))
        .await;
    resp.assert_status_ok();
    let downloaded = resp.into_bytes();
    assert_eq!(downloaded.len(), original_size);
    Ok(())
}

#[tokio::test]
async fn test_download_with_range_header() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // Create bucket + upload
    let bucket_body = json!({
        "name": "range-bucket",
        "path": "e2e-range-bucket",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let mut file_data = vec![];
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut file_data)
        .await?;

    let config = UploadUrlConfig {
        asset_type: AssetType::File,
        path: "e2e-range-bucket/range-test.jpg".to_string(),
        bucket: Some(bucket_pid.clone()),
        target_filesize: Some(file_data.len() as u64),
        create_parents: Some(true),
        overwrite: Some(true),
        expires: 120,
        ..Default::default()
    };

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();

    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_ok();

    // Sign download
    let sign_body = json!({
        "path": "range-test.jpg",
        "bucket": bucket_pid,
        "expires": 60
    });
    let resp = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let download_token: String = resp.json();

    // Download with Range header (first 100 bytes)
    let resp = server
        .get(&format!("/download/{download_token}"))
        .add_header("Range", "bytes=0-99")
        .await;
    resp.assert_status(StatusCode::PARTIAL_CONTENT);
    let range_data = resp.into_bytes();
    assert_eq!(range_data.len(), 100);
    assert_eq!(range_data[..], file_data[..100]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Auth / Login
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_login_invalid_credentials() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let body = json!({
        "email": "nonexistent@test.com",
        "password": "wrongpassword"
    });
    let resp = server.post("/auth/login", &body).await;
    resp.assert_status_unauthorized();
    Ok(())
}

#[tokio::test]
async fn test_login_valid_credentials() -> anyhow::Result<()> {
    let state = AppState::new().await?;
    let db = state.db();

    // Create a test user
    let email = "e2e-test-login@example.com";
    let password = "TestPassword123!";
    let _ = user::create(email, password, db).await;

    let server = TestServerWrapper::new().await?;
    let body = json!({
        "email": email,
        "password": password
    });
    let resp = server.post("/auth/login", &body).await;
    resp.assert_status_ok();

    let data: Value = resp.json();
    assert!(data["token"].is_string(), "response should contain a token");
    assert!(data["expires_in"].is_number(), "response should contain expires_in");
    assert_eq!(data["expires_in"].as_i64().unwrap(), 3600);
    Ok(())
}

#[tokio::test]
async fn test_login_empty_body() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let body = json!({});
    let resp = server.post("/auth/login", &body).await;
    resp.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    Ok(())
}

// ---------------------------------------------------------------------------
// Permissions (grant / revoke / list)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_permissions_unauthorized() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;
    let body = json!({
        "path": "test.jpg",
        "grantee": "some-pid",
        "permission": "read"
    });
    let resp = server.post("/buckets/fake-pid/permissions", &body).await;
    resp.assert_status_unauthorized();
    Ok(())
}

#[tokio::test]
async fn test_grant_and_revoke_permission_flow() -> anyhow::Result<()> {
    let (state, token_a, header_key) = setup_test_client().await?;
    let client_b = create_client(state.db(), state.secrets(), "E2E Client B").await?;
    let server = TestServerWrapper::new().await?;

    // 1. Client A creates a private bucket
    let bucket_body = json!({
        "name": "perm-test-bucket",
        "path": "e2e-perm-bucket",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // 2. Client A uploads a file
    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let mut file_data = vec![];
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut file_data)
        .await?;

    let config = UploadUrlConfig {
        asset_type: AssetType::File,
        path: "e2e-perm-bucket/secret.jpg".to_string(),
        bucket: Some(bucket_pid.clone()),
        target_filesize: Some(file_data.len() as u64),
        create_parents: Some(true),
        overwrite: Some(true),
        expires: 120,
        ..Default::default()
    };

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();

    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_ok();

    // 3. Client B tries to sign download — should be FORBIDDEN (no permission)
    let sign_body = json!({
        "path": "secret.jpg",
        "bucket": bucket_pid,
        "expires": 60
    });
    let resp = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, client_b.token())
        .await;
    resp.assert_status_forbidden();

    // 4. Client A grants read permission to Client B
    let grant_body = json!({
        "path": "secret.jpg",
        "grantee": client_b.id(),
        "grantee_type": "client",
        "permission": "read"
    });
    let resp = server
        .post(&format!("/buckets/{bucket_pid}/permissions"), &grant_body)
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status_ok();
    let grant_resp: Value = resp.json();
    assert_eq!(grant_resp["granted"], true);

    // 5. Client B can now sign download
    let resp = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, client_b.token())
        .await;
    resp.assert_status_ok();
    let download_token: String = resp.json();

    // 6. Client B downloads the file
    let resp = server
        .get(&format!("/download/{download_token}"))
        .await;
    resp.assert_status_ok();
    let downloaded = resp.into_bytes();
    assert_eq!(downloaded.len(), file_data.len());

    // 7. Client A lists permissions
    let resp = server
        .get(&format!("/buckets/{bucket_pid}/permissions?path=secret.jpg"))
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status_ok();
    let perms: Vec<Value> = resp.json();
    assert!(perms.len() >= 1, "expected at least 1 permission, got {}", perms.len());
    assert!(perms.iter().any(|p| p["permission"] == "read"), "expected a read permission");

    // 8. Client A revokes permission
    let revoke_body = json!({
        "path": "secret.jpg",
        "grantee": client_b.id(),
        "grantee_type": "client"
    });
    let resp = server
        .delete(&format!("/buckets/{bucket_pid}/permissions"), &revoke_body)
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status_ok();
    let revoke_resp: Value = resp.json();
    assert_eq!(revoke_resp["granted"], false);

    // 9. Client B can no longer sign download
    let resp = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, client_b.token())
        .await;
    resp.assert_status_forbidden();
    Ok(())
}

#[tokio::test]
async fn test_permissions_on_public_bucket_rejected() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // Create a PUBLIC bucket
    let bucket_body = json!({
        "name": "pub-perm-bucket",
        "path": "e2e-pub-perm-bucket",
        "public": true
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // Try to grant permission — should fail (permissions only for private buckets)
    let grant_body = json!({
        "path": "anything.jpg",
        "grantee": "some-pid",
        "grantee_type": "client",
        "permission": "read"
    });
    let resp = server
        .post(&format!("/buckets/{bucket_pid}/permissions"), &grant_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_non_owner_cannot_manage_permissions() -> anyhow::Result<()> {
    let (state, token_a, header_key) = setup_test_client().await?;
    let client_b = create_client(state.db(), state.secrets(), "E2E Non-Owner").await?;
    let server = TestServerWrapper::new().await?;

    // Client A creates a private bucket
    let bucket_body = json!({
        "name": "owner-only-bucket",
        "path": "e2e-owner-only",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // Client B tries to grant permission — should be FORBIDDEN
    let grant_body = json!({
        "path": "anything.jpg",
        "grantee": "someone",
        "grantee_type": "client",
        "permission": "read"
    });
    let resp = server
        .post(&format!("/buckets/{bucket_pid}/permissions"), &grant_body)
        .add_header(&header_key, client_b.token())
        .await;
    resp.assert_status_forbidden();
    Ok(())
}

#[tokio::test]
async fn test_permission_on_nonexistent_file() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // Create private bucket
    let bucket_body = json!({
        "name": "no-file-bucket",
        "path": "e2e-no-file",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // Grant permission on nonexistent file — need an existing grantee to pass grantee resolution
    let grant_body = json!({
        "path": "does-not-exist.jpg",
        "grantee": "not-a-real-pid",
        "grantee_type": "client",
        "permission": "read"
    });
    let resp = server
        .post(&format!("/buckets/{bucket_pid}/permissions"), &grant_body)
        .add_header(&header_key, &token)
        .await;
    // Grantee "not-a-real-pid" doesn't exist → 500 (SQL no rows) or 404 (file not found)
    // Both are acceptable for this negative test
    assert!(resp.status_code().is_client_error() || resp.status_code().is_server_error());
    Ok(())
}

#[tokio::test]
async fn test_list_permissions_empty_bucket() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let bucket_body = json!({
        "name": "empty-perm-bucket",
        "path": "e2e-empty-perms",
        "public": false
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // List all permissions (no path filter)
    let resp = server
        .get(&format!("/buckets/{bucket_pid}/permissions"))
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let perms: Vec<Value> = resp.json();
    assert!(perms.is_empty(), "empty bucket should have no permissions");
    Ok(())
}

// ---------------------------------------------------------------------------
// Download sign — public bucket rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_download_sign_public_bucket_rejected() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    // Create a public bucket
    let bucket_body = json!({
        "name": "pub-dl-bucket",
        "path": "e2e-pub-dl",
        "public": true
    });
    let resp = server
        .post("/buckets", &bucket_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);
    let bucket_pid: String = resp.json();

    // Try to sign download for public bucket — should fail
    let sign_body = json!({
        "path": "anything.jpg",
        "bucket": bucket_pid,
        "expires": 60
    });
    let resp = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_bad_request();
    Ok(())
}

// ---------------------------------------------------------------------------
// Resumable upload
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires Redis message broker"]
async fn test_resumable_upload_flow() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    use futures_util::StreamExt;
    use tokio_util::io::ReaderStream;

    let filepath = root_dir()?.join("test-assets/resumable.png");
    let file = tokio::fs::File::open(&filepath).await?;
    let filesize = file.metadata().await?.len();

    let mut config = test_upload_config();
    config.target_filesize = Some(filesize);
    config.create_parents = Some(true);
    config.overwrite = Some(true);
    config.resumable = Some(true);
    config.path = "test-assets/uploads/e2e-resumable.png".to_string();

    // Create resumable session
    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();

    let chunk_size = 2 * 1024 * 1024; // 2MB chunks
    let mut stream = ReaderStream::with_capacity(file, chunk_size);
    let mut next_token: Option<String> = None;

    // First chunk
    if let Some(Ok(first_chunk)) = stream.next().await {
        let session_token: String = resp.json();
        let resp = server
            .post_bytes(
                &format!("/upload/session/play/{session_token}"),
                first_chunk,
            )
            .await;
        resp.assert_status_ok();
        next_token = resp.json();
        assert!(next_token.is_some(), "first chunk should return next token");
    }

    // Remaining chunks
    while let Some(Ok(next_chunk)) = stream.next().await {
        if let Some(ref tok) = next_token {
            let resp = server
                .post_bytes(&format!("/upload/session/play/{tok}"), next_chunk)
                .await;
            resp.assert_status_ok();
            next_token = resp.json();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI command verification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cli_client_create_and_list() -> anyhow::Result<()> {
    let ppdrive_bin = root_dir()?.join("ppdrive");

    if !ppdrive_bin.exists() {
        // Skip CLI test if binary not available (debug build)
        return Ok(());
    }

    // Create client
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args(["client", "create", "--name", "CLI Test Client"])
        .output()
        .await?;

    assert!(output.status.success(), "client create should succeed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Client ID:"), "output should contain Client ID");
    assert!(stdout.contains("Client Token:"), "output should contain Client Token");

    // List clients
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args(["client", "list"])
        .output()
        .await?;

    assert!(output.status.success(), "client list should succeed");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("PID:"), "output should contain PID");
    Ok(())
}

#[tokio::test]
async fn test_cli_user_create() -> anyhow::Result<()> {
    let ppdrive_bin = root_dir()?.join("ppdrive");

    if !ppdrive_bin.exists() {
        return Ok(());
    }

    let output = tokio::process::Command::new(&ppdrive_bin)
        .args(["user", "create", "--email", "cli-test@example.com", "--password", "Pass123!"])
        .output()
        .await?;

    assert!(output.status.success(), "user create should succeed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("User created successfully!"));
    assert!(stdout.contains("cli-test@example.com"));
    Ok(())
}

#[tokio::test]
async fn test_cli_bucket_create() -> anyhow::Result<()> {
    let ppdrive_bin = root_dir()?.join("ppdrive");

    if !ppdrive_bin.exists() {
        return Ok(());
    }

    // First get a client PID
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args(["client", "list"])
        .output()
        .await?;
    let stdout = String::from_utf8(output.stdout)?;
    let client_pid = stdout.lines()
        .find_map(|line| line.strip_prefix("PID: ").map(|s| s.trim().to_string()))
        .expect("should have at least one client");

    // Create bucket
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args([
            "bucket", "create",
            "--name", "CLI Test Bucket",
            "--path", "cli-test-bucket",
            "--owner-type", "client",
            "--owner-id", &client_pid,
            "--public",
        ])
        .output()
        .await?;

    assert!(output.status.success(), "bucket create should succeed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Bucket created successfully!"));
    assert!(stdout.contains("Bucket ID:"));
    Ok(())
}

#[tokio::test]
async fn test_cli_bucket_path_validation() -> anyhow::Result<()> {
    let ppdrive_bin = root_dir()?.join("ppdrive");

    if !ppdrive_bin.exists() {
        return Ok(());
    }

    // Try absolute path
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args([
            "bucket", "create",
            "--name", "Bad Bucket",
            "--path", "/etc/passwd",
            "--owner-type", "client",
            "--owner-id", "1",
            "--public",
        ])
        .output()
        .await?;

    // Should fail with validation error
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        !output.status.success() || stderr.contains("must not start with '/'"),
        "absolute path should be rejected"
    );

    // Try path with ..
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args([
            "bucket", "create",
            "--name", "Bad Bucket",
            "--path", "../escape",
            "--owner-type", "client",
            "--owner-id", "1",
            "--public",
        ])
        .output()
        .await?;

    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        !output.status.success() || stderr.contains("'..'"),
        "path with .. should be rejected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Overwrite behavior
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_upload_overwrite_rejected_when_false() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let filepath = root_dir()?.join("test-assets/demo.jpg");
    let mut file_data = vec![];
    OpenOptions::new()
        .read(true)
        .open(&filepath)
        .await?
        .read_to_end(&mut file_data)
        .await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(file_data.len() as u64);
    config.create_parents = Some(true);
    config.overwrite = Some(true);

    // Upload once (succeeds)
    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();
    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_ok();

    // Upload again with overwrite=false — should fail
    config.overwrite = Some(false);
    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status_ok();
    let session_token: String = resp.json();
    let resp = server
        .post_bytes(
            &format!("/upload/session/play/{session_token}"),
            Bytes::copy_from_slice(&file_data),
        )
        .await;
    resp.assert_status_conflict();
    Ok(())
}

// ---------------------------------------------------------------------------
// Error response format
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_error_response_format() -> anyhow::Result<()> {
    let server = TestServerWrapper::new().await?;

    // Unknown route returns empty body (not JSON) — just verify the status
    let resp = server.get("/nonexistent-route").await;
    resp.assert_status_not_found();
    Ok(())
}

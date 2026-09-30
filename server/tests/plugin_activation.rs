//! The server must load only activated plugins and explain, at the gate,
//! why an unavailable plugin cannot be used.
//!
//! This lives in its own test binary so it gets its own process — and
//! therefore its own `LivePlugins` snapshot of `plugins.json`. The e2e
//! suite assumes no registry exists at the workspace root and must not
//! observe the entries written here.

mod common;

use axum::http::StatusCode;
use ppdrive::root_dir;
use ppdrive::server::{AssetType, ImageConversionConfig, UploadUrlConfig};
use serde_json::{Value, json};
use std::path::Path;

use crate::common::{TestServerWrapper, setup_test_client};

/// Write a registry with one deactivated plugin and one active plugin
/// whose library is missing. Skips when a registry already exists so a
/// developer install is never overwritten.
async fn write_test_registry() -> anyhow::Result<std::path::PathBuf> {
    let registry_path = root_dir()?.join("plugins.json");
    if registry_path.exists() {
        return Ok(registry_path);
    }

    let registry = json!({
        "plugins": [
            {
                "id": "image-transformation",
                "filename": "ppdrive-image-transformation-missing-linux-x86_64.so",
                "version": "1.0.0",
                "installed_at": "2026-01-01T00:00:00Z",
                "active": false
            },
            {
                "id": "image-conversion",
                "filename": "ppdrive-image-conversion-missing-linux-x86_64.so",
                "version": "1.0.0",
                "installed_at": "2026-01-01T00:00:00Z",
                "active": true
            }
        ]
    });
    tokio::fs::write(&registry_path, serde_json::to_vec_pretty(&registry)?).await?;
    Ok(registry_path)
}

#[tokio::test]
async fn test_registry_activation_gates_report_why() -> anyhow::Result<()> {
    let registry_path = write_test_registry().await?;
    if !Path::new(&registry_path).exists() {
        // Pre-existing developer registry: nothing to assert safely.
        return Ok(());
    }

    let (_, token, header_key) = setup_test_client().await?;
    // Booting must succeed even though an active entry's library is missing.
    let server = TestServerWrapper::new().await?;

    let health = server.get("/health").await;
    health.assert_status_ok();

    // Deactivated plugin: the gate names the state and the fix instead of
    // claiming the plugin is missing. Reaches the gate before the bucket
    // lookup, so the bucket PID is irrelevant.
    let sign_body = json!({
        "path": "photos/a.jpg",
        "bucket": "bkt-does-not-exist",
        "expires": 60,
        "image_transformation": {}
    });
    let sign = server
        .post("/download/sign", &sign_body)
        .add_header(&header_key, &token)
        .await;

    // Active but unloadable plugin: the upload gate reports the load
    // failure instead of claiming the plugin is missing.
    let upload_config = UploadUrlConfig {
        asset_type: AssetType::File,
        path: "test-assets/uploads/activation-test.jpg".to_string(),
        content_type: Some("image/jpeg".to_string()),
        target_filesize: Some(10),
        image_conversion: Some(ImageConversionConfig::default()),
        expires: 120,
        ..Default::default()
    };
    let upload = server
        .post("/upload/session", &upload_config)
        .add_header(&header_key, &token)
        .await;

    // Remove the registry before asserting so a failing assertion cannot
    // leave entries behind for other test binaries.
    tokio::fs::remove_file(&registry_path).await?;

    sign.assert_status(StatusCode::BAD_REQUEST);
    let sign_error: Value = sign.json();
    let sign_message = sign_error["error"].as_str().unwrap_or_default();
    assert!(
        sign_message.contains("deactivated"),
        "expected deactivated state in: {sign_message}"
    );
    assert!(
        sign_message.contains("ppdrive plugin activate"),
        "expected the activate hint in: {sign_message}"
    );

    upload.assert_status(StatusCode::BAD_REQUEST);
    let upload_error: Value = upload.json();
    let upload_message = upload_error["error"].as_str().unwrap_or_default();
    assert!(
        upload_message.contains("failed to load"),
        "expected load failure in: {upload_message}"
    );

    Ok(())
}

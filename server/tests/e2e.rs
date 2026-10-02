mod common;

use axum::body::Bytes;
use axum::http::StatusCode;
use ppdrive::AssetOwnerName;
use ppdrive::db::bucket;
use ppdrive::db::bucket::models::CreateBucketData;
use ppdrive::db::client::{create_client, get_id};
use ppdrive::db::user;
use ppdrive::root_dir;
use ppdrive::server::{AssetType, UploadUrlConfig};
use ppdrive::state::AppState;
use serde_json::{Value, json};
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;

use crate::common::{TestServerWrapper, setup_test_client};

fn test_upload_config() -> UploadUrlConfig {
    UploadUrlConfig {
        asset_type: AssetType::File,
        path: "test-assets/uploads/creator.jpg".to_string(),
        expires: 120,
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

/// A bucket path that would collide with a system API route is rejected at
/// creation — such a bucket could never be reached while serving.
#[tokio::test]
async fn test_create_bucket_reserved_path_rejected() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    for path in ["buckets", "auth", "assets"] {
        let body = json!({
            "name": "reserved-bucket",
            "path": path,
            "public": true
        });
        let resp = server
            .post("/buckets", &body)
            .add_header(&header_key, &token)
            .await;
        resp.assert_status(StatusCode::CONFLICT);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Live bucket serving (registry-backed; no restart)
// ---------------------------------------------------------------------------

/// A public bucket created *after* the app is already serving must be
/// reachable immediately: the create handler writes the registry through.
#[tokio::test]
async fn test_public_bucket_created_after_startup_serves_immediately() -> anyhow::Result<()> {
    let (state, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "name": "e2e-live-bucket",
        "path": "e2e-live-bucket",
        "public": true
    });
    let resp = server
        .post("/buckets", &body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);

    let dir = state.config().root_dir()?.join("e2e-live-bucket");
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join("hello.txt"), b"served without restart").await?;

    let resp = server.get("/e2e-live-bucket/hello.txt").await;
    resp.assert_status_ok();
    let body = resp.into_bytes();
    assert_eq!(&body[..], b"served without restart");
    Ok(())
}

/// A private bucket is never served directly at its path — downloads go
/// through the authenticated routes instead.
#[tokio::test]
async fn test_private_bucket_is_not_served_directly() -> anyhow::Result<()> {
    let (state, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "name": "e2e-private-bucket",
        "path": "e2e-private-bucket",
        "public": false
    });
    let resp = server
        .post("/buckets", &body)
        .add_header(&header_key, &token)
        .await;
    resp.assert_status(StatusCode::CREATED);

    let dir = state.config().root_dir()?.join("e2e-private-bucket");
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join("secret.txt"), b"not for the public").await?;

    let resp = server.get("/e2e-private-bucket/secret.txt").await;
    resp.assert_status(StatusCode::NOT_FOUND);
    Ok(())
}

/// The registry reconciles with the database: a row written outside the API
/// — exactly what `ppdrive bucket create` does — appears after `refresh`,
/// the same call the background reconciliation task makes.
#[tokio::test]
async fn test_bucket_registry_refresh_picks_up_direct_database_writes() -> anyhow::Result<()> {
    let (state, _token, _header_key) = setup_test_client().await?;

    let owner = create_client(state.db(), state.secrets(), "Refresh Owner").await?;
    let owner_id = get_id(owner.id(), state.db()).await?;
    let data = CreateBucketData {
        name: "refresh-bucket".into(),
        path: "/refresh-bucket".into(),
        owner_type: AssetOwnerName::Client,
        owner_id,
        public: true,
        size: None,
        accepts: None,
    };
    bucket::create(&data, &state.config().static_folders, state.db()).await?;

    // The API never ran, so the registry does not know the bucket yet.
    assert!(
        state
            .buckets()
            .longest_match("/refresh-bucket/f.txt")
            .is_none()
    );

    state.buckets().refresh(state.db()).await?;
    let (path, public) = state
        .buckets()
        .longest_match("/refresh-bucket/f.txt")
        .expect("refreshed registry should serve out-of-band bucket");
    assert_eq!(path, "/refresh-bucket");
    assert!(public);
    Ok(())
}

/// Config-driven static folders still serve files through the shared mount
/// logic. Skipped when no static folder is configured (a clean checkout has
/// no `ppd_config.toml`).
#[tokio::test]
async fn test_static_folder_serves_files() -> anyhow::Result<()> {
    let (state, _token, _header_key) = setup_test_client().await?;
    let Some(folder) = state.config().static_folders.first().cloned() else {
        return Ok(());
    };
    let server = TestServerWrapper::new().await?;

    let mount = folder.path.clone().unwrap_or(format!("/{}", folder.name));
    let dir = root_dir()?.join(&folder.name);
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join("e2e-static-note.txt"), b"static folder body").await?;

    let resp = server.get(&format!("{mount}/e2e-static-note.txt")).await;
    resp.assert_status_ok();
    let body = resp.into_bytes();
    assert_eq!(&body[..], b"static folder body");
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
    config.content_type = Some("image/jpeg".to_string());

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
        content_type: Some("image/jpeg".to_string()),
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
async fn test_image_conversion_requires_image_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = Some("application/pdf".to_string());
    config.image_conversion = Some(ppdrive::server::ImageConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_image_conversion_requires_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = None;
    config.image_conversion = Some(ppdrive::server::ImageConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a valid image-conversion request must be rejected
/// because the plugin is not installed.
#[tokio::test]
async fn test_image_conversion_plugin_not_installed() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/compress.jpg".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("image/jpeg".to_string());
    config.image_conversion = Some(ppdrive::server::ImageConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_image_transformation_requires_image_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = Some("application/pdf".to_string());
    config.image_transformation = Some(ppdrive::server::ImageTransformationConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_image_transformation_requires_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = None;
    config.image_transformation = Some(ppdrive::server::ImageTransformationConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

/// Operation arguments are validated before the plugin lookup, so an
/// invalid rotation is rejected even where the plugin isn't installed.
#[tokio::test]
async fn test_image_transformation_rejects_invalid_operation() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = Some("image/jpeg".to_string());
    config.image_transformation = Some(ppdrive::server::ImageTransformationConfig {
        operations: vec![ppdrive::server::TransformOperation::Rotate { degrees: 45 }],
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a valid image-transformation request must be rejected
/// because the plugin is not installed.
#[tokio::test]
async fn test_image_transformation_plugin_not_installed() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/transform.jpg".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("image/jpeg".to_string());
    config.image_transformation = Some(ppdrive::server::ImageTransformationConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    Ok(())
}

#[tokio::test]
async fn test_audio_conversion_requires_audio_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = Some("application/pdf".to_string());
    config.audio_conversion = Some(ppdrive::server::AudioConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("requires content_type to be an audio/* type")
    );
    Ok(())
}

#[tokio::test]
async fn test_audio_conversion_requires_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = None;
    config.audio_conversion = Some(ppdrive::server::AudioConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("content_type is required when audio_conversion is set")
    );
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a valid audio-conversion request must be rejected
/// because the plugin is not installed.
#[tokio::test]
async fn test_audio_conversion_plugin_not_installed() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/convert.wav".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("audio/wav".to_string());
    config.audio_conversion = Some(ppdrive::server::AudioConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("audio-conversion plugin is not installed")
    );
    Ok(())
}

#[tokio::test]
async fn test_audio_effects_requires_audio_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = Some("application/pdf".to_string());
    config.audio_effects = Some(ppdrive::server::AudioEffectsConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("requires content_type to be an audio/* type")
    );
    Ok(())
}

#[tokio::test]
async fn test_audio_effects_requires_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = None;
    config.audio_effects = Some(ppdrive::server::AudioEffectsConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("content_type is required when audio_effects is set")
    );
    Ok(())
}

/// Operation arguments are validated before the plugin lookup, so an
/// invalid speed factor is rejected even where the plugin isn't installed.
#[tokio::test]
async fn test_audio_effects_rejects_invalid_operation() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.target_filesize = Some(1024);
    config.content_type = Some("audio/mpeg".to_string());
    config.audio_effects = Some(ppdrive::server::AudioEffectsConfig {
        operations: vec![ppdrive::server::AudioEffectOperation::Speed { factor: 0.0 }],
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("audio_effects operation #0 (speed)")
    );
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a valid audio-effects request must be rejected
/// because the plugin is not installed.
#[tokio::test]
async fn test_audio_effects_plugin_not_installed() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/effects.mp3".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("audio/mpeg".to_string());
    config.audio_effects = Some(ppdrive::server::AudioEffectsConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("audio-effects plugin is not installed")
    );
    Ok(())
}

#[tokio::test]
async fn test_media_streaming_requires_media_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/stream.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("application/pdf".to_string());
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("media_streaming requires content_type to be a video/* or audio/* type")
    );
    Ok(())
}

#[tokio::test]
async fn test_media_streaming_requires_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/stream.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = None;
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("content_type is required when media_streaming is set")
    );
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a valid media-streaming request must be rejected
/// because the plugin is not installed.
#[tokio::test]
async fn test_media_streaming_plugin_not_installed() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/stream.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("media-streaming plugin is not installed")
    );
    Ok(())
}

#[tokio::test]
async fn test_video_conversion_requires_video_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/clip.jpg".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("image/jpeg".to_string());
    config.video_conversion = Some(ppdrive::server::VideoConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("video_conversion requires content_type to be a video/* type")
    );
    Ok(())
}

#[tokio::test]
async fn test_video_transformation_requires_content_type() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/clip.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = None;
    config.video_transformation = Some(ppdrive::server::VideoTransformationConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("content_type is required when video_transformation is set")
    );
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a valid video-conversion request must be rejected
/// because the plugin is not installed.
#[tokio::test]
async fn test_video_conversion_plugin_not_installed() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/clip.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.video_conversion = Some(ppdrive::server::VideoConversionConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("video-conversion plugin is not installed")
    );
    Ok(())
}

/// Operation arguments are rejected at session creation, before the
/// plugin gate.
#[tokio::test]
async fn test_video_transformation_rejects_invalid_rotate() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/clip.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.video_transformation = Some(ppdrive::server::VideoTransformationConfig {
        operations: vec![ppdrive::server::VideoTransformOperation::Rotate { degrees: 45 }],
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("degrees must be 90, 180 or 270")
    );
    Ok(())
}

/// `scale` is pre-flighted at session creation so a bad factor is a
/// client error, not a background-task failure.
#[tokio::test]
async fn test_video_conversion_rejects_zero_scale() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/clip.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.video_conversion = Some(ppdrive::server::VideoConversionConfig {
        scale: Some(0.0),
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("video_conversion scale must be finite and greater than 0")
    );
    Ok(())
}

/// Range validation on the option runs with the rest of the config
/// validation, before any media-plugin checks.
#[tokio::test]
async fn test_video_conversion_rejects_zero_fps() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/clip.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.video_conversion = Some(ppdrive::server::VideoConversionConfig {
        fps: Some(0),
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(!error["error"].as_str().unwrap_or_default().is_empty());
    Ok(())
}

#[tokio::test]
async fn test_media_streaming_output_dir_rejects_traversal() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/stream.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig {
        output_dir: Some("streams/../../escape".to_string()),
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    // output_dir is validated before the plugin gate, so the traversal
    // error surfaces even though no plugin is installed.
    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("path traversal detected")
    );
    Ok(())
}

#[tokio::test]
async fn test_media_streaming_output_dir_must_not_contain_upload_path() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/stream.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig {
        output_dir: Some("test-assets/uploads".to_string()),
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("output_dir must not contain the upload path")
    );
    Ok(())
}

#[tokio::test]
async fn test_media_streaming_rejects_bad_renditions() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.path = "test-assets/uploads/stream.mp4".to_string();
    config.target_filesize = Some(1024);
    config.content_type = Some("video/mp4".to_string());
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig {
        renditions: Some(Vec::new()),
        ..Default::default()
    });

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    // Renditions are validated before the plugin gate.
    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("renditions must not be empty")
    );
    Ok(())
}

#[tokio::test]
async fn test_media_streaming_only_applies_to_file_uploads() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let mut config = test_upload_config();
    config.asset_type = AssetType::Folder;
    config.path = "test-assets/uploads/folder".to_string();
    config.media_streaming = Some(ppdrive::server::MediaStreamingConfig::default());

    let resp = server
        .post("/upload/session", &config)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("media_streaming only applies to file uploads")
    );
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

    // Overwriting the same path must reuse the existing asset row instead of
    // violating UNIQUE (bucket_id, path).
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
    let resp = server.get(&format!("/download/{download_token}")).await;
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
// Image transformation on downloads
// ---------------------------------------------------------------------------

/// Percent-encode a JSON value for use as a query-string value.
fn encode_query_json(value: &Value) -> String {
    let raw = value.to_string();
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Register a public bucket backed by `e2e-transform-mount/` (gitignored),
/// drop an image and a text fixture inside it, then build the app. The row is
/// loaded into the serving registry at startup (buckets created later are
/// registered immediately through the API handler instead).
async fn setup_transform_mount() -> anyhow::Result<(TestServerWrapper, std::path::PathBuf)> {
    let (state, _token, _header_key) = setup_test_client().await?;

    let owner = create_client(state.db(), state.secrets(), "Transform Mount Owner").await?;
    let owner_id = get_id(owner.id(), state.db()).await?;
    let data = CreateBucketData {
        name: "e2e-transform-mount".into(),
        path: "/e2e-transform-mount".into(),
        owner_type: AssetOwnerName::Client,
        owner_id,
        public: true,
        size: None,
        accepts: None,
    };
    bucket::create(&data, &state.config().static_folders, state.db()).await?;

    let dir = root_dir()?.join("e2e-transform-mount");
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::copy(
        root_dir()?.join("test-assets/demo.jpg"),
        dir.join("img.jpg"),
    )
    .await?;
    tokio::fs::write(dir.join("notes.txt"), b"plain text fixture").await?;

    let server = TestServerWrapper::new().await?;
    Ok((server, dir))
}

/// Invalid operation arguments are rejected at sign time before any
/// database lookup — the bucket PID below does not exist.
#[tokio::test]
async fn test_sign_download_rejects_invalid_transformation_operation() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "path": "photos/a.jpg",
        "bucket": "bkt-does-not-exist",
        "expires": 60,
        "image_transformation": { "operations": [{ "rotate": { "degrees": 45 } }] }
    });
    let resp = server
        .post("/download/sign", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("rotate")
    );
    Ok(())
}

/// The transformation source must be an image, checked before the
/// database is touched.
#[tokio::test]
async fn test_sign_download_rejects_transformation_for_non_image() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "path": "docs/readme.txt",
        "bucket": "bkt-does-not-exist",
        "expires": 60,
        "image_transformation": { "operations": [] }
    });
    let resp = server
        .post("/download/sign", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("image/*")
    );
    Ok(())
}

/// The e2e environment runs from the workspace root, which has no
/// `plugins.json` — so a sign request with a valid transformation must be
/// rejected because the plugin is not installed (again, before the
/// database lookup).
#[tokio::test]
async fn test_sign_download_transformation_requires_plugin() -> anyhow::Result<()> {
    let (_, token, header_key) = setup_test_client().await?;
    let server = TestServerWrapper::new().await?;

    let body = json!({
        "path": "photos/a.jpg",
        "bucket": "bkt-does-not-exist",
        "expires": 60,
        "image_transformation": {}
    });
    let resp = server
        .post("/download/sign", &body)
        .add_header(&header_key, &token)
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("not installed")
    );
    Ok(())
}

/// Requests without the parameter must pass through the wrapper untouched
/// and be served by `ServeDir` as before.
#[tokio::test]
async fn test_direct_download_passthrough_without_transformation() -> anyhow::Result<()> {
    let (server, _) = setup_transform_mount().await?;

    let original = tokio::fs::read(root_dir()?.join("test-assets/demo.jpg")).await?;
    let resp = server.get("/e2e-transform-mount/img.jpg").await;

    resp.assert_status_ok();
    assert_eq!(resp.into_bytes().len(), original.len());
    Ok(())
}

/// Malformed transformation JSON is rejected with a client-facing message.
#[tokio::test]
async fn test_direct_download_rejects_malformed_transformation() -> anyhow::Result<()> {
    let (server, _) = setup_transform_mount().await?;

    let resp = server
        .get("/e2e-transform-mount/img.jpg?image_transformation=%7Bnot-valid-json")
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("invalid image_transformation")
    );
    Ok(())
}

/// The unauthenticated surface only accepts typed operations — raw FFmpeg
/// filter strings are rejected.
#[tokio::test]
async fn test_direct_download_rejects_custom_filters() -> anyhow::Result<()> {
    let (server, _) = setup_transform_mount().await?;

    let query = encode_query_json(&json!({ "custom_filters": "eq=brightness=0.1" }));
    let resp = server
        .get(&format!(
            "/e2e-transform-mount/img.jpg?image_transformation={query}"
        ))
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("custom_filters")
    );
    Ok(())
}

/// A request-target containing a literal `..` segment is rejected with a
/// clear 400. The axum-test client normalizes dot segments before sending
/// (WHATWG URL rules), so this test speaks HTTP/1.1 over a raw socket to
/// deliver the unnormalized path.
#[tokio::test]
async fn test_direct_download_rejects_parent_traversal() -> anyhow::Result<()> {
    let (server, _) = setup_transform_mount().await?;
    let port = server.port();

    let response = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        use std::io::{Read, Write};

        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        stream.write_all(
            concat!(
                "GET /e2e-transform-mount/../e2e-transform-mount/img.jpg",
                "?image_transformation=%7B%7D HTTP/1.1\r\n",
                "Host: 127.0.0.1\r\n",
                "Connection: close\r\n",
                "\r\n"
            )
            .as_bytes(),
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    })
    .await??;

    assert!(
        response.starts_with("HTTP/1.1 400"),
        "expected 400, got: {response}"
    );
    assert!(
        response.contains("'..' is not allowed"),
        "unexpected response: {response}"
    );
    Ok(())
}

/// Only image files can be transformed on the direct surface; a text file
/// under the same mount is rejected on content type.
#[tokio::test]
async fn test_direct_download_requires_image_source() -> anyhow::Result<()> {
    let (server, _) = setup_transform_mount().await?;

    let resp = server
        .get("/e2e-transform-mount/notes.txt?image_transformation=%7B%7D")
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("image/*")
    );
    Ok(())
}

/// A conditional request with a matching ETag is answered with `304 Not
/// Modified` without invoking the plugin — the key is derived from the
/// source file and the requested configuration.
#[tokio::test]
async fn test_direct_download_revalidates_with_etag() -> anyhow::Result<()> {
    let (server, dir) = setup_transform_mount().await?;

    let file = std::fs::canonicalize(dir.join("img.jpg"))?;
    let metadata = std::fs::metadata(&file)?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let config = ppdrive::server::ImageTransformationConfig::default();
    let etag = config.cache_key(&file.to_string_lossy(), metadata.len(), mtime);

    let query = encode_query_json(&json!({ "operations": [] }));
    let resp = server
        .get(&format!(
            "/e2e-transform-mount/img.jpg?image_transformation={query}"
        ))
        .add_header("If-None-Match", format!("\"{etag}\""))
        .await;

    resp.assert_status(StatusCode::NOT_MODIFIED);
    assert_eq!(
        resp.headers().get(axum::http::header::ETAG),
        Some(&axum::http::HeaderValue::from_str(&format!("\"{etag}\""))?)
    );
    Ok(())
}

/// A well-formed request must be rejected at the plugin gate (the e2e
/// environment has no `image-transformation` plugin installed).
#[tokio::test]
async fn test_direct_download_transformation_requires_plugin() -> anyhow::Result<()> {
    let (server, _) = setup_transform_mount().await?;

    let resp = server
        .get("/e2e-transform-mount/img.jpg?image_transformation=%7B%7D")
        .await;

    resp.assert_status_bad_request();
    let error: Value = resp.json();
    assert!(
        error["error"]
            .as_str()
            .unwrap_or_default()
            .contains("not installed")
    );
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
    assert!(
        data["expires_in"].is_number(),
        "response should contain expires_in"
    );
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
    let resp = server.get(&format!("/download/{download_token}")).await;
    resp.assert_status_ok();
    let downloaded = resp.into_bytes();
    assert_eq!(downloaded.len(), file_data.len());

    // 7. Client A lists permissions
    let resp = server
        .get(&format!(
            "/buckets/{bucket_pid}/permissions?path=secret.jpg"
        ))
        .add_header(&header_key, &token_a)
        .await;
    resp.assert_status_ok();
    let perms: Vec<Value> = resp.json();
    assert!(
        !perms.is_empty(),
        "expected at least 1 permission, got {}",
        perms.len()
    );
    assert!(
        perms.iter().any(|p| p["permission"] == "read"),
        "expected a read permission"
    );

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
    config.content_type = Some("image/png".to_string());

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

    assert!(
        output.status.success(),
        "client create should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("Client ID:"),
        "output should contain Client ID"
    );
    assert!(
        stdout.contains("Client Token:"),
        "output should contain Client Token"
    );

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
        .args([
            "user",
            "create",
            "--email",
            "cli-test@example.com",
            "--password",
            "Pass123!",
        ])
        .output()
        .await?;

    assert!(
        output.status.success(),
        "user create should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    let client_pid = stdout
        .lines()
        .find_map(|line| line.strip_prefix("PID: ").map(|s| s.trim().to_string()))
        .expect("should have at least one client");

    // Create bucket
    let output = tokio::process::Command::new(&ppdrive_bin)
        .args([
            "bucket",
            "create",
            "--name",
            "CLI Test Bucket",
            "--path",
            "cli-test-bucket",
            "--owner-type",
            "client",
            "--owner-id",
            &client_pid,
            "--public",
        ])
        .output()
        .await?;

    assert!(
        output.status.success(),
        "bucket create should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
            "bucket",
            "create",
            "--name",
            "Bad Bucket",
            "--path",
            "/etc/passwd",
            "--owner-type",
            "client",
            "--owner-id",
            "1",
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
            "bucket",
            "create",
            "--name",
            "Bad Bucket",
            "--path",
            "../escape",
            "--owner-type",
            "client",
            "--owner-id",
            "1",
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
    config.content_type = Some("image/jpeg".to_string());

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

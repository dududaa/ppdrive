//! Upload and download session types, and password hashing.
//!
//! Defines [`UploadInfo`] (the signed upload session token), [`DownloadInfo`]
//! (the signed download token), [`UploadUrlConfig`]
//! (client-provided upload parameters), and Argon2 password helpers.

use crate::db::Database;
use crate::hasher::{Hashable, Hasher, errors::PayloadVerificationError};
use crate::utils;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::Path;
use validator::Validate;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct UploadInfo {
    pub client_id: String,
    pub session_id: Option<String>,
    pub exp: i64,
    pub chunk_index: u16,
    /// This can be derived from [UploadUrlConfig]'s `expires` property and later used by broker
    /// to determine resumable chunk's url expiration.
    pub chunk_session_expiration: i64,
    pub config: Option<UploadUrlConfig>,
    /// The decrypted client signing key, populated when creating a session.
    /// Not serialized into the signed token for security.
    #[serde(skip)]
    pub client_key: Option<String>,
}

impl UploadInfo {
    /// Sign this [`UploadInfo`] with the given key and hasher, returning a base64url token.
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    /// Increment the chunk index, clear config, re-expire, and re-sign.
    pub fn resign(&mut self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        self.chunk_index += 1;
        self.config = None;
        // Use chunk_session_expiration (relative duration) not self.exp (absolute timestamp)
        self.exp = utils::seconds_from_now(self.chunk_session_expiration)?;

        self.sign(key, hasher)
    }

    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<UploadInfo, PayloadVerificationError> {
        hasher.verify_upload_info(signed, db, secrets).await
    }
}

impl Hashable for UploadInfo {
    #[allow(clippy::manual_async_fn)]
    fn key(&self) -> impl Future<Output = anyhow::Result<String>> {
        async {
            // If client_key is already populated (e.g., from create_session), use it directly
            if let Some(key) = &self.client_key {
                return Ok(key.clone());
            }
            // Otherwise, look up and decrypt from database
            Err(anyhow!(
                "client_key not available on deserialized UploadInfo; use verify_with_secrets"
            ))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

/// Signed token for accessing a file in a private bucket.
#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadInfo {
    pub client_id: String,
    pub path: String,
    pub bucket_pid: String,
    pub exp: i64,
    /// Decrypted client signing key. Never serialized into the token.
    #[serde(skip)]
    pub client_key: Option<String>,
}

impl DownloadInfo {
    /// Sign this [`DownloadInfo`] with the given key and hasher, returning a base64url token.
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    /// Verify a signed download token, returning the decoded [`DownloadInfo`].
    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<DownloadInfo, PayloadVerificationError> {
        hasher.verify_download_info(signed, db, secrets).await
    }
}

impl Hashable for DownloadInfo {
    #[allow(clippy::manual_async_fn)]
    fn key(&self) -> impl Future<Output = anyhow::Result<String>> {
        async {
            if let Some(key) = &self.client_key {
                return Ok(key.clone());
            }
            Err(anyhow!(
                "client_key not available on deserialized DownloadInfo; use verify_download_info"
            ))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

/// Signed token for user authentication.
#[derive(Serialize, Deserialize, Clone)]
pub struct UserInfo {
    pub user_email: String,
    pub exp: i64,
}

impl UserInfo {
    /// Sign this [`UserInfo`] with the given key and hasher, returning a base64url token.
    pub fn sign(&self, key: &str, hasher: &Hasher) -> anyhow::Result<String> {
        hasher.hash(key, self)
    }

    /// Verify a signed user token, returning the decoded [`UserInfo`].
    pub async fn verify(
        signed: &str,
        db: &Database,
        secrets: &crate::tools::secrets::AppSecrets,
        hasher: &Hasher,
    ) -> Result<UserInfo, PayloadVerificationError> {
        hasher.verify_user_info(signed, db, secrets).await
    }
}

impl Hashable for UserInfo {
    #[allow(clippy::manual_async_fn)]
    fn key(&self) -> impl Future<Output = anyhow::Result<String>> {
        async {
            // User tokens are signed with the app secret, not a per-user key
            Err(anyhow!(
                "UserInfo::key() should not be called directly; use verify_user_info instead"
            ))
        }
    }

    fn expires(&self) -> i64 {
        self.exp
    }
}

/// Request body for `POST /download/sign`.
#[derive(Serialize, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct SignDownloadRequest {
    /// Relative path of the file within the bucket.
    #[validate(length(min = 1, max = 2048))]
    pub path: String,
    /// PID of the private bucket containing the file.
    #[validate(length(min = 1, max = 64))]
    pub bucket: String,
    /// Token lifetime in seconds (30–3600).
    #[validate(range(min = 30, max = 3600))]
    pub expires: i64,
}

#[derive(Serialize, Deserialize, Validate, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct UploadUrlConfig {
    pub asset_type: AssetType,
    #[validate(range(min = 30, max = 86400))]
    pub expires: i64,
    #[validate(length(min = 4, max = 2048))]
    pub path: String,
    /// expect filesize for this upload
    pub target_filesize: Option<u64>,
    /// Create asset parent folders if they don't exist, else error will be returned.
    pub create_parents: Option<bool>,
    /// overwrite asset if it already exists.
    pub overwrite: Option<bool>,
    pub resumable: Option<bool>,
    /// The bucket to which the asset belongs
    #[validate(length(min = 1, max = 64))]
    pub bucket: Option<String>,
    /// MIME type of the file being uploaded (e.g. "image/png").
    /// Required when the target bucket has an `accepts` restriction.
    #[validate(length(min = 1, max = 128))]
    pub content_type: Option<String>,
    /// MIME types accepted for this upload (e.g. ["image/png", "image/*"]).
    /// Required when `bucket` is not provided.
    #[validate(length(max = 20))]
    pub accepts: Option<Vec<String>>,
    /// Whether the uploaded file should be publicly accessible.
    /// Only effective for files in private buckets. Defaults to false.
    pub public: Option<bool>,
    /// Post-upload image compression. Requires the `image_compression`
    /// plugin and an `image/*` `content_type`.
    #[validate(nested)]
    pub image_compression: Option<ImageCompressionConfig>,
}

impl UploadUrlConfig {
    #[cfg(test)]
    pub fn test() -> Self {
        UploadUrlConfig {
            asset_type: AssetType::File,
            path: "test-assets/uploads/creator.jpg".to_string(),
            expires: 120,
            ..Default::default()
        }
    }
}

/// Output format for post-upload image compression.
///
/// Variant names and serde representation match
/// `image_compression::ImageFormat` so the JSON serialized here
/// deserializes into the plugin's `CompressionOptions`.
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    #[default]
    Jpeg,
    Png,
    WebP,
    Avif,
}

impl ImageFormat {
    /// File extension used when this format is written to storage.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::WebP => "webp",
            ImageFormat::Avif => "avif",
        }
    }

    /// MIME type of this format.
    pub fn mime(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Avif => "image/avif",
        }
    }
}

/// Client-provided options for compressing the file after upload,
/// applied by the `image_compression` plugin.
#[derive(Serialize, Deserialize, Validate, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct ImageCompressionConfig {
    /// Output format. Default: [`ImageFormat::Jpeg`].
    pub format: ImageFormat,
    /// Quality on a 0–100 scale (values above 100 are rejected here;
    /// the plugin clamps at 100). Default: 80.
    #[validate(range(min = 0, max = 100))]
    pub quality: u8,
    /// Target width in pixels; `None` keeps the source width. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub width: Option<u32>,
    /// Target height in pixels; `None` keeps the source height. Must be ≥ 1.
    #[validate(range(min = 1))]
    pub height: Option<u32>,
    /// Compress after responding (`true`) or before (`false`, inline default).
    /// Falls back to `image_compression_background` in `ppd_config.toml`.
    pub background: Option<bool>,
}

impl Default for ImageCompressionConfig {
    fn default() -> Self {
        ImageCompressionConfig {
            format: ImageFormat::Jpeg,
            quality: 80,
            width: None,
            height: None,
            background: None,
        }
    }
}

impl ImageCompressionConfig {
    /// Rewrite `path`'s extension to match the output format
    /// (e.g. `images/photo.png` → `images/photo.webp`).
    pub fn rewrite_path(&self, path: &str) -> String {
        Path::new(path)
            .with_extension(self.format.extension())
            .to_string_lossy()
            .into_owned()
    }
}

/// Resolve whether compression runs in the background:
/// client option → global config → inline (`false`).
pub fn resolve_background(client: Option<bool>, global: Option<bool>) -> bool {
    client.or(global).unwrap_or(false)
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum AssetType {
    #[default]
    File,
    Folder,
}

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::db::{Database, client};
    use crate::hasher::Hasher;
    use crate::secrets::AppSecrets;
    use crate::server::{UploadInfo, UploadUrlConfig};
    use crate::utils::seconds_from_now;
    use std::sync::Arc;
    use tokio::sync::{Mutex, OnceCell};

    type SharedConfig = Arc<Mutex<AppConfig>>;
    static APP_CONFIG: OnceCell<SharedConfig> = OnceCell::const_new();

    async fn get_config() -> &'static SharedConfig {
        APP_CONFIG
            .get_or_init(|| async {
                let config = AppConfig::read().await.unwrap();
                Arc::new(Mutex::new(config))
            })
            .await
    }

    async fn run_sign_info_test(config: AppConfig) -> anyhow::Result<()> {
        AppSecrets::init().await?;
        let secrets = AppSecrets::read().await?;

        let db = Database::new(&config.database_url, 10).await?;
        let hasher = config.hasher.clone();
        let client_details = client::create_client(&db, &secrets, "Signed Client").await?;

        let config = UploadUrlConfig::test();
        let plaintext_key = client::get_key(&db, client_details.id(), &secrets).await?;
        let info = UploadInfo {
            client_id: client_details.id().to_string(),
            exp: seconds_from_now(config.expires)?,
            config: Some(config),
            client_key: Some(plaintext_key),
            ..Default::default()
        };

        let key = info.client_key.clone().unwrap();
        let mut signed = info.sign(&key, &hasher)?;

        let mut verified = UploadInfo::verify(&signed, &db, &secrets, &hasher).await;
        assert!(verified.is_ok());

        // is tampered, this should fail
        signed.push_str("mod");
        verified = UploadInfo::verify(&signed, &db, &secrets, &hasher).await;
        assert!(verified.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_info_hmac256_signing() -> anyhow::Result<()> {
        let config = get_config().await.lock().await;
        run_sign_info_test(config.clone()).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_info_blake3_signing() -> anyhow::Result<()> {
        let mut config = get_config().await.lock().await;
        config.hasher = Hasher::Blake3;

        run_sign_info_test(config.clone()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod image_compression_tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn image_compression_applies_defaults() {
        let json = r#"{
            "asset_type": "File",
            "expires": 120,
            "path": "images/a.png",
            "image_compression": { "format": "WebP", "width": 800 }
        }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        let ic = config.image_compression.unwrap();
        assert_eq!(ic.format, ImageFormat::WebP);
        assert_eq!(ic.quality, 80);
        assert_eq!(ic.width, Some(800));
        assert_eq!(ic.height, None);
        assert_eq!(ic.background, None);
    }

    #[test]
    fn image_compression_missing_field_parses_to_none() {
        let json = r#"{ "asset_type": "File", "expires": 120, "path": "images/a.png" }"#;
        let config: UploadUrlConfig = serde_json::from_str(json).unwrap();
        assert!(config.image_compression.is_none());
    }

    #[test]
    fn image_compression_serializes_all_dispatch_fields() {
        let config = ImageCompressionConfig {
            format: ImageFormat::Avif,
            quality: 60,
            width: Some(100),
            height: None,
            background: Some(true),
        };
        let value = serde_json::to_value(&config).unwrap();
        for key in ["format", "quality", "width", "height", "background"] {
            assert!(value.get(key).is_some(), "missing field '{key}'");
        }
        assert_eq!(value["format"], "Avif");
        assert_eq!(value["quality"], 60);
    }

    #[test]
    fn image_compression_rejects_unknown_fields() {
        let json = r#"{
            "format": "Jpeg", "quality": 80, "width": null,
            "height": null, "background": null, "fuzzy": true
        }"#;
        assert!(serde_json::from_str::<ImageCompressionConfig>(json).is_err());
    }

    #[test]
    fn image_compression_validation_rules() {
        let mut config = UploadUrlConfig::test();

        config.image_compression = Some(ImageCompressionConfig {
            quality: 101,
            ..Default::default()
        });
        assert!(config.validate().is_err(), "quality 101 must be rejected");

        config.image_compression = Some(ImageCompressionConfig {
            width: Some(0),
            ..Default::default()
        });
        assert!(config.validate().is_err(), "width 0 must be rejected");

        config.image_compression = Some(ImageCompressionConfig {
            quality: 100,
            width: Some(1),
            height: Some(1),
            ..Default::default()
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn resolve_background_precedence() {
        assert!(!resolve_background(None, None), "default is inline");
        assert!(resolve_background(None, Some(true)));
        assert!(
            !resolve_background(Some(false), Some(true)),
            "client option overrides global"
        );
        assert!(resolve_background(Some(true), Some(false)));
        assert!(resolve_background(Some(true), None));
    }

    #[test]
    fn rewrite_path_uses_output_extension() {
        let webp = ImageCompressionConfig {
            format: ImageFormat::WebP,
            ..Default::default()
        };
        assert_eq!(webp.rewrite_path("images/photo.png"), "images/photo.webp");
        assert_eq!(webp.rewrite_path("photo"), "photo.webp");
        assert_eq!(webp.rewrite_path("a.b/photo.jpg"), "a.b/photo.webp");

        let avif = ImageCompressionConfig {
            format: ImageFormat::Avif,
            ..Default::default()
        };
        assert_eq!(avif.rewrite_path("photo.jpg"), "photo.avif");
    }

    #[test]
    fn format_extension_and_mime() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Jpeg.mime(), "image/jpeg");
        assert_eq!(ImageFormat::Png.mime(), "image/png");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
        assert_eq!(ImageFormat::Avif.extension(), "avif");
        assert_eq!(ImageFormat::Avif.mime(), "image/avif");
    }
}

//! Application configuration (TOML).
//!
//! Parses `ppd_config.toml` and provides [`AppConfig`] with sensible defaults.

use crate::db;
#[cfg(feature = "server")]
use crate::hasher::Hasher;
use std::collections::HashMap;

use crate::paths_cross;
use crate::root_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_FILENAME: &str = "ppd_config.toml";

#[derive(Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server_name: String,
    pub database_url: String,
    pub client_header_key: String,
    pub allowed_origins: Option<Vec<String>>,
    pub port: Option<u16>,
    pub root_dir: Option<String>,
    pub message_broker: Option<String>,
    #[serde(default)]
    pub static_folders: Vec<StaticFolder>,
    /// Max database connections in the pool (default: 10).
    pub db_pool_size: Option<u32>,

    #[cfg(feature = "server")]
    pub hasher: Hasher,

    pub plugins: Option<HashMap<String, HashMap<String, String>>>,

    /// Default for whether image conversion runs after the upload response
    /// (`true`) or inline before it (`false`). Per-upload
    /// `image_conversion.background` overrides this.
    #[serde(default, alias = "image_compression_background")]
    pub image_conversion_background: Option<bool>,

    /// Default for whether image transformation runs after the upload
    /// response (`true`) or inline before it (`false`). Per-upload
    /// `image_transformation.background` overrides this.
    #[serde(default)]
    pub image_transformation_background: Option<bool>,

    /// Default for whether audio conversion runs after the upload response
    /// (`true`) or inline before it (`false`). Per-upload
    /// `audio_conversion.background` overrides this.
    #[serde(default)]
    pub audio_conversion_background: Option<bool>,

    /// Default for whether audio effects run after the upload response
    /// (`true`) or inline before it (`false`). Per-upload
    /// `audio_effects.background` overrides this.
    #[serde(default)]
    pub audio_effects_background: Option<bool>,

    /// Default for whether video conversion runs after the upload response
    /// (`true`) or inline before it (`false`). Per-upload
    /// `video_conversion.background` overrides this.
    #[serde(default)]
    pub video_conversion_background: Option<bool>,

    /// Default for whether video transformation runs after the upload
    /// response (`true`) or inline before it (`false`). Per-upload
    /// `video_transformation.background` overrides this.
    #[serde(default)]
    pub video_transformation_background: Option<bool>,

    /// Default for whether media streaming packaging runs after the
    /// upload response (`true`) or inline before it (`false`). Per-upload
    /// `media_streaming.background` overrides this.
    #[serde(default)]
    pub media_streaming_background: Option<bool>,

    /// TTL in seconds for transformed-download cache entries stored in the
    /// message broker. `0` disables server-side caching entirely.
    /// Requires `message_broker` to be configured. Defaults to 86400 (24h).
    #[serde(default)]
    pub transform_cache_ttl_secs: Option<u64>,

    /// HTTP `Cache-Control: max-age` in seconds for transformed-download
    /// responses. Defaults to 3600 (1h).
    #[serde(default)]
    pub transform_cache_max_age_secs: Option<u64>,

    /// Seconds between background reconciliations of the in-memory bucket
    /// registry with the database. `0` disables background reloads. Buckets
    /// created through the API are registered immediately regardless of this
    /// setting; the reconciliation covers writers that bypass the API (the
    /// CLI, another instance sharing the database). Defaults to 15.
    #[serde(default)]
    pub bucket_reload_interval: Option<u64>,
}

impl AppConfig {
    /// Load the application configuration from `ppd_config.toml`.
    ///
    /// Falls back to [`AppConfig::default`] if the file is missing or unreadable,
    /// logging a warning in either case.
    pub async fn read() -> anyhow::Result<Self> {
        let filename = config_filename()?;
        let config = match tokio::fs::read_to_string(&filename).await {
            Ok(content) => toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", filename.display()))?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let config = AppConfig::default();
                if let Err(save_err) = config.save().await {
                    tracing::warn!(
                        "failed to create default config file {}: {save_err}",
                        filename.display()
                    );
                } else {
                    tracing::info!("created default config file {}", filename.display());
                }
                config
            }
            Err(err) => {
                tracing::warn!(
                    "failed to read {}: {err}, using defaults",
                    filename.display()
                );
                AppConfig::default()
            }
        };

        Ok(config)
    }

    /// Resolve the storage root directory, optionally appending a user-configured sub-path.
    pub fn root_dir(&self) -> anyhow::Result<PathBuf> {
        match &self.root_dir {
            Some(dir) => Ok(root_dir()?.join(dir)),
            None => Ok(root_dir()?),
        }
    }

    /// Persist the current configuration to `ppd_config.toml`.
    pub async fn save(&self) -> anyhow::Result<()> {
        let filename = config_filename()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("failed to serialize config: {e}"))?;
        tokio::fs::write(&filename, content).await?;
        Ok(())
    }

    /// Remove any static folder whose path crosses an existing bucket path,
    /// log the details, and save the updated configuration.
    pub async fn validate_static_folders(&mut self, db: &db::Database) -> anyhow::Result<()> {
        let bucket_paths = db::bucket::get_all_paths(db).await?;
        let mut removed = Vec::new();

        self.static_folders.retain(|folder| {
            let default_path = format!("/{}", folder.name);
            let folder_path = folder.path.as_deref().unwrap_or(&default_path);

            for bucket_path in &bucket_paths {
                if paths_cross(folder_path, bucket_path) {
                    tracing::warn!(
                        "Removing static folder '{}' (path: '{folder_path}') — conflicts with existing bucket path '{bucket_path}'",
                        folder.name
                    );
                    removed.push((folder.name.clone(), folder_path.to_string(), bucket_path.clone()));
                    return false;
                }
            }
            true
        });

        if !removed.is_empty() {
            tracing::info!(
                "Removed {} static folder(s) due to path conflicts with existing buckets",
                removed.len()
            );
            self.save().await?;
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_name: "ppdrive-prod-01".to_string(),
            database_url: "sqlite:data.db".to_string(),
            client_header_key: "x-ppdrive-client".to_string(),
            allowed_origins: None,
            port: Some(8000),
            root_dir: None,
            message_broker: None,
            static_folders: vec![],
            db_pool_size: Some(10),
            #[cfg(feature = "server")]
            hasher: Hasher::HMAC256,
            plugins: None,
            image_conversion_background: None,
            image_transformation_background: None,
            audio_conversion_background: None,
            audio_effects_background: None,
            video_conversion_background: None,
            video_transformation_background: None,
            media_streaming_background: None,
            transform_cache_ttl_secs: None,
            transform_cache_max_age_secs: None,
            bucket_reload_interval: None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct StaticFolder {
    pub name: String,
    pub path: Option<String>,
}

fn config_filename() -> anyhow::Result<PathBuf> {
    let path = root_dir()?.join(CONFIG_FILENAME);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_image_conversion_background() {
        let toml_content = r#"
database_url = "sqlite:data.db"
client_header_key = "x-ppdrive-client"
hasher = "HMAC256"
image_conversion_background = true
image_transformation_background = true
audio_conversion_background = true
audio_effects_background = true
video_conversion_background = true
video_transformation_background = true
media_streaming_background = true
"#;
        let config: AppConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.image_conversion_background, Some(true));
        assert_eq!(config.image_transformation_background, Some(true));
        assert_eq!(config.audio_conversion_background, Some(true));
        assert_eq!(config.audio_effects_background, Some(true));
        assert_eq!(config.video_conversion_background, Some(true));
        assert_eq!(config.video_transformation_background, Some(true));
        assert_eq!(config.media_streaming_background, Some(true));
    }

    #[test]
    fn legacy_image_compression_background_still_parses() {
        // Config files written before the rename keep working.
        let toml_content = r#"
database_url = "sqlite:data.db"
client_header_key = "x-ppdrive-client"
hasher = "HMAC256"
image_compression_background = true
"#;
        let config: AppConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.image_conversion_background, Some(true));
    }

    #[test]
    fn image_conversion_background_defaults_to_none() {
        let toml_content = r#"
database_url = "sqlite:data.db"
client_header_key = "x-ppdrive-client"
hasher = "HMAC256"
"#;
        let config: AppConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.image_conversion_background, None);
        assert_eq!(config.image_transformation_background, None);
        assert_eq!(AppConfig::default().image_conversion_background, None);
        assert_eq!(AppConfig::default().image_transformation_background, None);
        assert_eq!(config.audio_conversion_background, None);
        assert_eq!(config.audio_effects_background, None);
        assert_eq!(AppConfig::default().audio_conversion_background, None);
        assert_eq!(AppConfig::default().audio_effects_background, None);
        assert_eq!(config.video_conversion_background, None);
        assert_eq!(config.video_transformation_background, None);
        assert_eq!(AppConfig::default().video_conversion_background, None);
        assert_eq!(AppConfig::default().video_transformation_background, None);
        assert_eq!(config.media_streaming_background, None);
        assert_eq!(AppConfig::default().media_streaming_background, None);
    }

    #[test]
    fn parses_transform_cache_settings() {
        let with_values = r#"
database_url = "sqlite:data.db"
client_header_key = "x-ppdrive-client"
hasher = "HMAC256"
transform_cache_ttl_secs = 300
transform_cache_max_age_secs = 60
"#;
        let config: AppConfig = toml::from_str(with_values).unwrap();
        assert_eq!(config.transform_cache_ttl_secs, Some(300));
        assert_eq!(config.transform_cache_max_age_secs, Some(60));

        // Absent fields fall back to None (callers apply defaults), and 0 is
        // preserved so it can disable the cache.
        let disabled = r#"
database_url = "sqlite:data.db"
client_header_key = "x-ppdrive-client"
hasher = "HMAC256"
transform_cache_ttl_secs = 0
"#;
        let config: AppConfig = toml::from_str(disabled).unwrap();
        assert_eq!(config.transform_cache_ttl_secs, Some(0));
        assert_eq!(config.transform_cache_max_age_secs, None);
        assert_eq!(AppConfig::default().transform_cache_ttl_secs, None);
    }
}

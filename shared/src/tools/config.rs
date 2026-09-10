//! Application configuration (TOML).
//!
//! Parses `ppd_config.toml` and provides [`AppConfig`] with sensible defaults.

use crate::db;
#[cfg(feature = "server")]
use crate::hasher::Hasher;

use crate::paths_cross;
use crate::root_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_FILENAME: &str = "ppd_config.toml";

#[derive(Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub client_header_key: String,
    pub allowed_origins: Option<Vec<String>>,
    pub port: Option<u16>,
    pub root_dir: Option<String>,
    pub message_broker: Option<String>,
    pub static_folders: Vec<StaticFolder>,
    /// Max database connections in the pool (default: 10).
    pub db_pool_size: Option<u32>,

    #[cfg(feature = "server")]
    pub hasher: Hasher,
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
                tracing::warn!("config file {} not found, using defaults", filename.display());
                AppConfig::default()
            }
            Err(err) => {
                tracing::warn!("failed to read {}: {err}, using defaults", filename.display());
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
    pub async fn validate_static_folders(
        &mut self,
        db: &db::Database,
    ) -> anyhow::Result<()> {
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

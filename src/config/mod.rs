use axum::http::HeaderValue;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;

pub mod configor;
pub mod secrets;

pub(super) const CONFIG_FILENAME: &str = "ppd_config.toml";

#[derive(Deserialize, Serialize)]
pub struct BaseConfig {
    pub(super) port: u16,
    pub(super) allowed_origins: String,
    pub(super) database_url: String,
    debug_mode: bool,
}

impl BaseConfig {
    pub fn port(&self) -> &u16 {
        &self.port
    }

    pub fn allowed_origins(&self) -> Vec<HeaderValue> {
        let origins = &self.allowed_origins;
        origins.split(",").flat_map(|o| {
            match o.parse::<HeaderValue>() {
                Ok(h) => Some(h),
                Err(err) => {
                    tracing::warn!("unable to parse origin {o}. Origin will not be whitelisted. \nmore info: {err}");
                    None
                }
            }
        }).collect()
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn debug_mode(&self) -> &bool {
        &self.debug_mode
    }
}

#[derive(Deserialize, Serialize)]
pub struct FileUploadConfig {
    pub(super) max_upload_size: usize,
}

impl FileUploadConfig {
    pub fn max_upload_size(&self) -> &usize {
        &self.max_upload_size
    }
}

#[derive(Deserialize, Serialize)]
pub struct AppConfig {
    pub(super) base: BaseConfig,
    file_upload: FileUploadConfig,
}

impl AppConfig {
    pub async fn build() -> Result<Self, AppError> {
        let config_str = tokio::fs::read_to_string(CONFIG_FILENAME).await?;
        let config: Self =
            toml::from_str(&config_str).map_err(|err| AppError::InitError(err.to_string()))?;

        Ok(config)
    }

    pub fn base(&self) -> &BaseConfig {
        &self.base
    }

    pub fn file_upload(&self) -> &FileUploadConfig {
        &self.file_upload
    }
}

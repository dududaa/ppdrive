use std::path::PathBuf;

use axum::http::HeaderValue;
use serde::{Deserialize, Serialize};
use tower_http::cors::AllowOrigin;

use crate::{errors::AppError, utils::install_dir};

pub mod configor;
pub mod secrets;

pub(super) const CONFIG_FILENAME: &str = "ppd_config.toml";

#[derive(Deserialize, Serialize)]
pub struct BaseConfig {
    port: u16,
    allowed_origins: String,
    db_url: String,
}

impl BaseConfig {
    pub fn port(&self) -> &u16 {
        &self.port
    }

    pub fn allowed_origins(&self) -> AllowOrigin {
        let origins = &self.allowed_origins;
        if origins == "*" {
            AllowOrigin::any()
        } else {
            let list: Vec<HeaderValue> = origins.split(",").flat_map(|o| {
                match o.parse::<HeaderValue>() {
                    Ok(h) => Some(h),
                    Err(err) => {
                        tracing::warn!("unable to parse origin {o}. Origin will not be whitelisted. \nmore info: {err}");
                        None
                    }
                }
            }).collect();

            list.into()
        }
    }

    pub fn db_url(&self) -> &str {
        &self.db_url
    }
}

#[derive(Deserialize, Serialize)]
pub struct FileUploadConfig {
    max_upload_size: usize,
}

impl FileUploadConfig {
    pub fn max_upload_size(&self) -> &usize {
        &self.max_upload_size
    }
}

#[derive(Deserialize, Serialize)]
pub struct AppConfig {
    base: BaseConfig,
    file_upload: FileUploadConfig,
}

impl AppConfig {
    pub async fn build() -> Result<Self, AppError> {
        let config_path = Self::config_path()?;

        let config_str = tokio::fs::read_to_string(&config_path).await?;
        let config: Self =
            toml::from_str(&config_str).map_err(|err| AppError::InitError(err.to_string()))?;

        Ok(config)
    }

    pub fn config_path() -> Result<PathBuf, AppError> {
        let path = if cfg!(debug_assertions) {
            CONFIG_FILENAME.into()
        } else {
            install_dir()?.join(CONFIG_FILENAME)
        };

        Ok(path)
    }

    pub fn base(&self) -> &BaseConfig {
        &self.base
    }

    pub fn file_upload(&self) -> &FileUploadConfig {
        &self.file_upload
    }
}

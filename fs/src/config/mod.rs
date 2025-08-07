use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{CoreResult, config::auth::AuthConfig, errors::CoreError, tools::install_dir};
pub mod auth;

pub const CONFIG_FILENAME: &str = "ppd_config.toml";

pub enum CorsOriginType {
    Any,
    List(Vec<String>),
}

pub fn get_config_path() -> CoreResult<PathBuf> {
    let path = if cfg!(debug_assertions) {
        CONFIG_FILENAME.into()
    } else {
        install_dir()?.join(CONFIG_FILENAME)
    };

    Ok(path)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    url: String,
}

impl DatabaseConfig {
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    port: u16,
    max_upload_size: usize,
    allowed_origins: String,
}

impl ServerConfig {
    pub fn port(&self) -> &u16 {
        &self.port
    }

    pub fn max_upload_size(&self) -> &usize {
        &self.max_upload_size
    }

    pub fn allowed_origins(&self) -> &str {
        &self.allowed_origins
    }

    pub fn origins(&self) -> CorsOriginType {
        let c = &self.allowed_origins;
        let list: Vec<&str> = c.split(",").collect();

        match list.first() {
            Some(first) => {
                if first == &"*" {
                    CorsOriginType::Any
                } else {
                    let list = list.iter().map(|o| o.to_string()).collect();
                    CorsOriginType::List(list)
                }
            }
            None => CorsOriginType::Any,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AppConfig {
    database: DatabaseConfig,
    server: ServerConfig,
    auth: AuthConfig,
}

impl AppConfig {
    pub async fn load(config_path: PathBuf) -> CoreResult<Self> {
        let config_str = tokio::fs::read_to_string(&config_path).await?;
        let config: Self =
            toml::from_str(&config_str).map_err(|err| CoreError::ServerError(err.to_string()))?;

        Ok(config)
    }

    pub fn db(&self) -> &DatabaseConfig {
        &self.database
    }

    pub fn server(&self) -> &ServerConfig {
        &self.server
    }

    pub fn auth(&self) -> &AuthConfig {
        &self.auth
    }

    pub async fn update(&mut self, data: ConfigUpdater) -> CoreResult<()> {
        // database
        let url = &self.database.url;
        self.database.url = data.db_url.unwrap_or(url.to_string());

        // server
        let port = &self.server.port;
        let origins = &self.server.allowed_origins;
        let max_upload = &self.server.max_upload_size;

        self.server.port = data.server_port.unwrap_or(port.clone());
        self.server.allowed_origins = data.allowed_urls.unwrap_or(origins.to_string());
        self.server.max_upload_size = data.max_upload_size.unwrap_or(max_upload.clone());

        // save to file
        let updated =
            toml::to_string_pretty(&self).map_err(|err| CoreError::ServerError(err.to_string()))?;
        let path = get_config_path()?;

        tokio::fs::write(&path, &updated).await?;

        Ok(())
    }
}

#[derive(Default)]
pub struct ConfigUpdater {
    pub db_url: Option<String>,
    pub server_port: Option<u16>,
    pub max_upload_size: Option<usize>,
    pub allowed_urls: Option<String>,
}

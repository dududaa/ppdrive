use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{config::auth::AuthConfig, errors::Error, plugins::service::ServiceAuthMode, tools::root_dir, AppResult};
pub mod auth;

pub const CONFIG_FILENAME: &str = "ppd_config.toml";

pub enum CorsOriginType {
    Any,
    List(Vec<String>),
}

fn get_config_path() -> AppResult<PathBuf> {
    let path = root_dir()?.join(CONFIG_FILENAME);
    Ok(path)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    url: String,
    manager_port: u16
}

impl DatabaseConfig {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn manager_addr(&self) -> String {
        format!("0.0.0.0:{}", self.manager_port)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    max_upload_size: usize,
    allowed_origins: String,
    auth_modes: Vec<ServiceAuthMode>
}

impl ServerConfig {
    pub fn max_upload_size(&self) -> &usize {
        &self.max_upload_size
    }

    pub fn allowed_origins(&self) -> &str {
        &self.allowed_origins
    }

    pub fn auth_modes(&self) -> &[ServiceAuthMode] {
        &self.auth_modes
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
    pub async fn load() -> AppResult<Self> {
        let config_path = get_config_path()?;
        let config_str = tokio::fs::read_to_string(&config_path).await.map_err(|_| Error::ServerError(format!("unable to read {config_path:?}")))?;
        let config: Self =
            toml::from_str(&config_str).map_err(|err| Error::ServerError(err.to_string()))?;

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

    pub async fn set_auth_modes(&mut self, values: &[ServiceAuthMode]) -> AppResult<()> {
        self.server.auth_modes = values.to_vec();
        let updated =
        toml::to_string_pretty(&self).map_err(|err| Error::ServerError(err.to_string()))?;
        
        let path = get_config_path()?;
        tokio::fs::write(&path, &updated).await?;
        Ok(())
    }
}
use crate::root_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_FILENAME: &str = "ppd_config.toml";

#[derive(Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub client_header_key: String,
    pub allowed_origins: Option<Vec<String>>,
    pub port: Option<i16>,
    pub root_dir: Option<String>,
    pub use_session: bool,
    pub static_folders: Vec<StaticFolder>,
}
impl AppConfig {
    pub async fn read() -> anyhow::Result<Self> {
        let filename = config_filename()?;
        let content = match tokio::fs::read_to_string(filename).await {
            Ok(content) => content,
            Err(_) => serde_json::to_string(&AppConfig::default())?,
        };

        let config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn root_dir(&self) -> anyhow::Result<PathBuf> {
        match &self.root_dir {
            Some(dir) => Ok(root_dir()?.join(dir)),
            None => Ok(root_dir()?),
        }
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
            use_session: false,
            static_folders: vec![],
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

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_FILENAME: &'static str = "ppd_config.toml";

#[derive(Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub client_header_key: String,
    pub allowed_origins: Option<Vec<String>>,
    pub port: Option<i16>,
}

impl AppConfig {
    pub async fn read() -> anyhow::Result<Self> {
        let filename = config_filename()?;
        let content = tokio::fs::read_to_string(filename)
            .await
            .map_err(|err| anyhow!("unable to get app config: {}", err))?;

        let config = toml::from_str(&content)?;
        Ok(config)
    }
}

fn config_filename() -> anyhow::Result<PathBuf> {
    let path = crate::root_dir()?.join(CONFIG_FILENAME);
    Ok(path)
}

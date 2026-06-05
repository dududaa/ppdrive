use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{SqlitePool, migrate};

pub mod client;
mod models;
mod tools;

pub use tools::*;

pub type DbPool = SqlitePool;

pub async fn create_pool(url: &str) -> anyhow::Result<DbPool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await?;

    migrate!("../migrations").run(&pool).await?;
    Ok(pool)
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub client_header_key: String,
    pub allowed_origins: Option<Vec<String>>,
    pub port: Option<i16>,
}

impl AppConfig {
    pub async fn read() -> anyhow::Result<Self> {
        let content = tokio::fs::read_to_string("ppd_config.toml").await?;
        let config = toml::from_str(&content)?;

        Ok(config)
    }
}

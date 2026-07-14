use std::str::FromStr;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{migrate, SqlitePool};

pub mod client;
mod models;
mod tools;

pub use tools::*;

pub type DbPool = SqlitePool;

pub async fn create_pool(url: &str) -> anyhow::Result<DbPool> {
    let pool = sqlite_pool(url).await?;
    migrate!("../migrations").run(&pool).await?;
    Ok(pool)
}

async fn sqlite_pool(url: &str) -> anyhow::Result<DbPool> {
    let connection_options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(connection_options)
        .await?;

    Ok(pool)
}

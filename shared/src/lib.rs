use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{migrate, SqlitePool};

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


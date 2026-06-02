use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

mod models;
mod tools;

pub use tools::*;

type DbPool = SqlitePool;

pub async fn create_pool(url: &str) -> anyhow::Result<DbPool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await?;

    Ok(pool)
}

use sqlx::any::AnyPoolOptions;
use sqlx::AnyPool;

pub mod models;

pub async fn create_pool(url: &str) -> anyhow::Result<AnyPool> {
    let pool = AnyPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await?;

    Ok(pool)
}

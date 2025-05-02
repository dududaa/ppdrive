use crate::{errors::AppError, utils::get_env};
use sqlx::{
    any::{install_default_drivers, AnyPoolOptions},
    AnyPool,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn create_db_pool() -> Result<AnyPool, AppError> {
    let debug_mode = get_env("DEBUG_MODE")?;
    if &debug_mode != "true" {
        // run_migrations().await?;
    }

    install_default_drivers();
    let connection_url = get_env("DATABASE_URL")?;
    let pool = AnyPoolOptions::new()
        .max_connections(100)
        .connect(&connection_url)
        .await
        .map_err(|err| AppError::InitError(err.to_string()))?;

    Ok(pool)
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<AnyPool>>,
}

impl AppState {
    pub async fn new() -> Result<Self, AppError> {
        let pool = create_db_pool().await?;
        let db = Arc::new(Mutex::new(pool));
        let s = Self { db };

        Ok(s)
    }

    pub async fn pool(&self) -> DbPool {
        self.db.lock().await.clone()
    }
}

use crate::{
    config::AppConfig,
    errors::AppError,
    utils::{get_env, sqlx_utils::BackendName},
};
use sqlx::{
    any::{install_default_drivers, AnyPoolOptions},
    AnyPool,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn create_db_pool() -> Result<AnyPool, AppError> {
    let debug_mode = get_env("DEBUG_MODE")?;
    let connection_url = get_env("DATABASE_URL")?;

    if &debug_mode != "true" {
        // run_migrations().await?;
    }

    install_default_drivers();
    let pool = AnyPoolOptions::new()
        .connect(&connection_url)
        .await
        .map_err(|err| AppError::InitError(err.to_string()))?;

    Ok(pool)
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<AnyPool>>,
    config: Arc<AppConfig>,
    backend_name: BackendName,
}

impl AppState {
    pub async fn new() -> Result<Self, AppError> {
        let pool = create_db_pool().await?;

        let conn = pool.acquire().await?;
        let db = Arc::new(Mutex::new(pool));

        let backend_name = conn.backend_name().try_into()?;
        let config = Arc::new(AppConfig::build().await?);
        let s = Self {
            db,
            backend_name,
            config,
        };

        Ok(s)
    }

    pub async fn db_pool(&self) -> AnyPool {
        self.db.lock().await.clone()
    }

    pub fn backend_name(&self) -> &BackendName {
        &self.backend_name
    }

    pub fn config(&self) -> Arc<AppConfig> {
        self.config.clone()
    }
}

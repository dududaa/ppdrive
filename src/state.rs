use crate::{
    config::{secrets::AppSecrets, AppConfig},
    errors::AppError,
    utils::sqlx::sqlx_utils::BackendName,
};
use sqlx::{
    any::{install_default_drivers, AnyPoolOptions},
    AnyPool,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn create_db_pool(config: &AppConfig) -> Result<AnyPool, AppError> {
    let connection_url = config.base().database_url();

    install_default_drivers();
    let pool = AnyPoolOptions::new()
        .connect(&connection_url)
        .await
        .map_err(|err| AppError::InitError(err.to_string()))?;

    if !cfg!(debug_assertions) {
        sqlx::migrate!()
            .run(&pool)
            .await
            .map_err(|err| AppError::InitError(err.to_string()))?;
    }

    Ok(pool)
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<AnyPool>>,
    config: Arc<AppSecrets>,
    backend_name: BackendName,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> Result<Self, AppError> {
        let pool = create_db_pool(config).await?;

        let conn = pool.acquire().await?;
        let db = Arc::new(Mutex::new(pool));

        let backend_name = conn.backend_name().try_into()?;
        let config = Arc::new(AppSecrets::read().await?);
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

    pub fn config(&self) -> Arc<AppSecrets> {
        self.config.clone()
    }
}

use crate::errors::AppError;
use ppdrive_core::{config::AppConfig, db::init_db, tools::secrets::AppSecrets, RBatis};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    db: RBatis,
    secrets: Arc<AppSecrets>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> Result<Self, AppError> {
        let db = init_db(config.db().url()).await?;

        let config = Arc::new(AppSecrets::read().await?);
        let s = Self {
            db,
            secrets: config,
        };

        Ok(s)
    }

    pub fn db(&self) -> &RBatis {
        &self.db
    }

    pub fn secrets(&self) -> Arc<AppSecrets> {
        self.secrets.clone()
    }
}

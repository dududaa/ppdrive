use crate::{
    config::{secrets::AppSecrets, AppConfig},
    errors::AppError,
};
use ppdrive_core::{db::init_db, RBatis};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    db: RBatis,
    config: Arc<AppSecrets>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> Result<Self, AppError> {
        let db = init_db(config.base().db_url()).await?;

        let config = Arc::new(AppSecrets::read().await?);
        let s = Self { db, config };

        Ok(s)
    }

    pub fn db(&self) -> &RBatis {
        &self.db
    }

    pub fn config(&self) -> Arc<AppSecrets> {
        self.config.clone()
    }
}

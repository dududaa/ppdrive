use crate::errors::RestError;
use ppdrive_fs::{config::AppConfig, db::init_db, tools::secrets::AppSecrets, RBatis};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    db: RBatis,
    secrets: Arc<AppSecrets>,
    config: Arc<AppConfig>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> Result<Self, RestError> {
        let db = init_db(config.db().url()).await?;
        let secrets = Arc::new(AppSecrets::read().await?);

        let config = Arc::new(config.clone());

        let s = Self {
            db,
            secrets,
            config,
        };

        Ok(s)
    }

    pub fn db(&self) -> &RBatis {
        &self.db
    }

    pub fn secrets(&self) -> Arc<AppSecrets> {
        self.secrets.clone()
    }

    pub fn config(&self) -> Arc<AppConfig> {
        self.config.clone()
    }
}

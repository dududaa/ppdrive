use crate::errors::HandlerError;
use ppd_bk::{db::init_db, RBatis};
use ppd_shared::{config::AppConfig, tools::AppSecrets};
use std::sync::Arc;

#[derive(Clone)]
pub struct HandlerState {
    db: RBatis,
    secrets: Arc<AppSecrets>,
    config: Arc<AppConfig>,
}

impl HandlerState {
    pub async fn new(config: &AppConfig) -> Result<Self, HandlerError> {
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

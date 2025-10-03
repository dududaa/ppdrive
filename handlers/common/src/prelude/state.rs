use crate::errors::HandlerError;
use ppd_bk::RBatis;
use ppd_shared::{opts::ServiceConfig, tools::AppSecrets};
use std::sync::Arc;

#[derive(Clone)]
pub struct HandlerState {
    db: Arc<RBatis>,
    secrets: Arc<AppSecrets>,
    config: Arc<ServiceConfig>,
}

impl HandlerState {
    pub async fn new(config: &ServiceConfig, db: Arc<RBatis>) -> Result<Self, HandlerError> {
        let secrets = AppSecrets::read().await?;
        let secrets = Arc::new(secrets);
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

    pub fn config(&self) -> Arc<ServiceConfig> {
        self.config.clone()
    }
}

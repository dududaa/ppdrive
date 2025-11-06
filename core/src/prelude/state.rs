use crate::errors::HandlerError;
use ppd_bk::{RBatis, db::init_db};
use ppd_shared::{opts::internal::ServiceConfig, tools::AppSecrets};
use std::sync::Arc;

#[derive(Clone)]
pub struct HandlerState {
    db: Arc<RBatis>,
    secrets: Arc<AppSecrets>,
    config: Arc<ServiceConfig>,
}

impl HandlerState {
    pub async fn new(config: &ServiceConfig) -> Result<Self, HandlerError> {
        tracing::debug!("reading secrets...");
        let secrets = AppSecrets::read().await?;
        let secrets = Arc::new(secrets);

        tracing::debug!("making config...");
        let config = Arc::new(config.clone());

        tracing::debug!("reading secrets...");
        let db = init_db(&config.base.db_url, true).await?;

        let s = Self {
            db: Arc::new(db),
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

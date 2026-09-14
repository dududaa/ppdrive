use crate::config::AppConfig;
use crate::db::{Database, DbPool};
use crate::secrets::AppSecrets;
use crate::broker::MessageBroker;
use crate::hasher::Hasher;

#[derive(Clone)]
pub struct AppState {
    secrets: AppSecrets,
    config: AppConfig,
    db: Database,
    broker: Option<MessageBroker>,
}

impl AppState {
    /// Core-only constructor (no broker, no static folder validation).
    pub async fn new() -> anyhow::Result<Self> {
        let config = AppConfig::read().await?;
        AppSecrets::init().await?;
        let secrets = AppSecrets::read().await?;
        let db = Database::new(&config.database_url, config.db_pool_size.unwrap_or(10)).await?;
        Ok(Self {
            secrets,
            config,
            db,
            broker: None,
        })
    }

    /// Full constructor with broker and static folder validation (server use).
    pub async fn with_broker() -> anyhow::Result<Self> {
        let mut config = AppConfig::read().await?;
        AppSecrets::init().await?;
        let secrets = AppSecrets::read().await?;
        let db = Database::new(&config.database_url, config.db_pool_size.unwrap_or(10)).await?;
        config.validate_static_folders(&db).await?;
        let broker = match &config.message_broker {
            Some(url) => Some(MessageBroker::new(url).await?),
            None => None,
        };
        Ok(Self {
            secrets,
            config,
            db,
            broker,
        })
    }

    pub fn secrets(&self) -> &AppSecrets {
        &self.secrets
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn pool(&self) -> &DbPool {
        &self.db
    }

    pub fn broker(&self) -> anyhow::Result<&MessageBroker> {
        self.broker
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("broker not configured"))
    }

    pub fn hasher(&self) -> &Hasher {
        &self.config.hasher
    }
}

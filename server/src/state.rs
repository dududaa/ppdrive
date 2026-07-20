use shared::broker::MessageBroker;
use shared::config::AppConfig;
use shared::db::{Database, DbPool};
use shared::hasher::Hasher;
use shared::secrets::AppSecrets;

#[derive(Clone)]
pub struct AppState {
    secrets: AppSecrets,
    config: AppConfig,
    db: Database,
    broker: Option<MessageBroker>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let config = AppConfig::read().await?;
        let secrets = AppSecrets::read().await?;

        let db = Database::new(&config.database_url).await?;
        let mut broker = None;
        if let Some(url) = &config.message_broker {
            broker = Some(MessageBroker::new(url).await?);
        }

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
        match self.broker {
            Some(ref broker) => Ok(broker),
            None => Err(anyhow::anyhow!("broker not found.")),
        }
    }

    pub fn hasher(&self) -> &Hasher {
        &self.config().hasher
    }
}

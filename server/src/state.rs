use shared::config::AppConfig;
use shared::db::{Database, DbPool};
use shared::secrets::AppSecrets;

#[derive(Clone)]
pub struct AppState {
    secrets: AppSecrets,
    config: AppConfig,
    pool: Database
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let config = AppConfig::read().await?;
        let secrets = AppSecrets::read().await?;
        let pool = Database::new(&config.database_url).await?;

        Ok(Self { secrets, config, pool })
    }

    pub fn secrets(&self) -> &AppSecrets {
        &self.secrets
    }
    
    pub fn config(&self) -> &AppConfig {
        &self.config
    }
    
    pub fn db(&self) -> &Database {
        &self.pool
    }
    
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}
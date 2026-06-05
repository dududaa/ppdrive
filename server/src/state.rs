use shared::{create_pool, AppConfig, AppSecrets, DbPool};

#[derive(Clone)]
pub struct AppState {
    secrets: AppSecrets,
    config: AppConfig,
    pool: DbPool
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let config = AppConfig::read().await?;
        let secrets = AppSecrets::read().await?;
        let pool = create_pool(&config.database_url).await?;
        
        Ok(Self { secrets, config, pool })
    }
    
    pub fn secrets(&self) -> &AppSecrets {
        &self.secrets
    }
    
    pub fn config(&self) -> &AppConfig {
        &self.config
    }
    
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}
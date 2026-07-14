use std::ops::Deref;
use sqlx::{AnyPool, migrate};
use sqlx::any::install_default_drivers;

pub type DbPool = AnyPool;

#[derive(Clone)]
pub struct Database {
    pool: DbPool,
    engine: DbEngine,
}

impl Database {
    pub async fn new(url: &str) -> anyhow::Result<Self> {
        install_default_drivers();
        let pool = AnyPool::connect(url).await?;
        
        let engine = if url.starts_with("postgres") {
            DbEngine::Postgres
        } else if url.starts_with("mysql") {
            DbEngine::Mysql
        } else {
            DbEngine::Sqlite
        };

        migrate!("../migrations").run(&pool).await?;
        Ok(Self { pool, engine })
    }

    pub fn placeholder(&self, idx: u8) -> String {
        match self.engine {
            DbEngine::Mysql => "?".to_string(),
            _ => format!("${idx}")
        }
    }
}

impl Deref for Database {
    type Target = DbPool;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

#[derive(Clone)]
pub enum DbEngine {
    Sqlite,
    Postgres,
    Mysql,
}

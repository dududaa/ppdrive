use std::ops::Deref;
use std::str::FromStr;
use sqlx::{AnyPool, migrate};
use sqlx::any::install_default_drivers;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

pub type DbPool = AnyPool;

#[derive(Clone)]
pub struct Database {
    pool: DbPool,
    engine: DbEngine,
}

impl Database {
    pub async fn new(url: &str) -> anyhow::Result<Self> {
        install_default_drivers();

        let engine = if url.starts_with("postgres") {
            DbEngine::Postgres
        } else if url.starts_with("mysql") {
            DbEngine::Mysql
        } else {
            create_sqlite(url).await?;
            DbEngine::Sqlite
        };

        let pool = AnyPool::connect(url).await?;
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

/// Forces the creation of sqlite file (in case the URL is a file)
async fn create_sqlite(url: &str) -> anyhow::Result<()> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true);

    let _ = SqlitePoolOptions::new().connect_with(options).await?;
    Ok(())
}
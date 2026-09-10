//! Database abstraction layer.
//!
//! Wraps sqlx [`AnyPool`] to support SQLite, PostgreSQL, and MySQL backends.
//! Handles connection setup, migrations, and engine-specific placeholder syntax.

use std::ops::Deref;
use std::str::FromStr;
use std::time::Duration;
use sqlx::{AnyPool, migrate};
use sqlx::pool::PoolOptions;
use sqlx::any::install_default_drivers;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

pub mod bucket;
pub mod client;
pub mod user;

pub type DbPool = AnyPool;

#[derive(Clone)]
pub struct Database {
    pool: DbPool,
    engine: DbEngine,
}

impl Database {
    /// Create a new database connection pool, run migrations, and detect the engine type.
    pub async fn new(url: &str, max_connections: u32) -> anyhow::Result<Self> {
        install_default_drivers();

        let engine = if url.starts_with("postgres") {
            DbEngine::Postgres
        } else if url.starts_with("mysql") {
            DbEngine::Mysql
        } else {
            create_sqlite(url).await?;
            DbEngine::Sqlite
        };

        let pool = PoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(300))
            .connect(url)
            .await?;
        migrate!("../migrations").run(&pool).await?;
        Ok(Self { pool, engine })
    }

    /// Return the engine-specific placeholder (`?` for MySQL, `$N` otherwise).
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

/// Forces the creation of sqlite file (in case the URL is a file) and enables WAL mode.
async fn create_sqlite(url: &str) -> anyhow::Result<()> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let _ = SqlitePoolOptions::new().connect_with(options).await?;
    Ok(())
}
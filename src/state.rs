use std::sync::Arc;

use diesel::{Connection, PgConnection};
use diesel_async::{
    pooled_connection::{bb8::{Pool, PooledConnection}, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tokio::sync::Mutex;

use crate::{errors::AppError, utils::get_env};

type DbPool = Pool<AsyncPgConnection>;
pub type DbPooled<'a> = PooledConnection<'a, AsyncPgConnection>;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

async fn run_migrations() -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        let database_url =
            get_env("DATABASE_URL").expect("unable to get database url from environment");

        let mut conn =
            PgConnection::establish(&database_url).expect("failed to connect to database");
        conn.run_pending_migrations(MIGRATIONS)
            .expect("failed to run migration");
    });

    Ok(())
}

pub async fn create_db_pool() -> Result<DbPool, AppError> {
    let debug_mode = get_env("DEBUG_MODE")?;
    if &debug_mode != "true" {
        run_migrations().await?;
    }

    let connection_url = get_env("DATABASE_URL")?;
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(connection_url);
    let pool = Pool::builder()
        .build(config)
        .await
        .map_err(|err| AppError::InitError(err.to_string()))?;

    Ok(pool)
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<DbPool>>,
}

impl AppState {
    pub async fn new() -> Result<Self, AppError> {
        let pool = create_db_pool().await?;
        let db = Arc::new(Mutex::new(pool));
        let s = Self { db };

        Ok(s)
    }

    pub async fn pool(&self) -> DbPool {
        self.db.lock().await.clone()
    }
}
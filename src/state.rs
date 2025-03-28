use std::sync::Arc;

use diesel_async::{
    pooled_connection::{bb8::{Pool, PooledConnection}, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use tokio::sync::Mutex;

use crate::{errors::AppError, utils::get_env};

type DbPool = Pool<AsyncPgConnection>;
pub type DbPooled<'a> = PooledConnection<'a, AsyncPgConnection>;

pub async fn create_db_pool() -> Result<DbPool, AppError> {
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
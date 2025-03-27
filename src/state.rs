use std::sync::Arc;

use diesel_async::{
    pooled_connection::{bb8::Pool, AsyncDieselConnectionManager},
    AsyncPgConnection,
};
use tokio::sync::Mutex;

use crate::{errors::PPDriveError, utils::get_env};

type DbPool = Pool<AsyncPgConnection>;

async fn create_db_pool() -> Result<DbPool, PPDriveError> {
    let connection_url = get_env("DATABASE_URL")?;
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(connection_url);
    let pool = Pool::builder()
        .build(config)
        .await
        .map_err(|err| PPDriveError::InitError(err.to_string()))?;

    Ok(pool)
}

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<DbPool>>,
}

impl AppState {
    pub async fn new() -> Result<Self, PPDriveError> {
        let pool = create_db_pool().await?;
        let db = Arc::new(Mutex::new(pool));
        let s = Self { db };

        Ok(s)
    }
}
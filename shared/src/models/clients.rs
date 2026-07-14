use crate::DbPool;
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use serde::Serialize;
use sqlx::FromRow;

#[derive(FromRow)]
pub struct Client {
    pid: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: DateTime<Utc>,
}


impl Client {
    pub async fn create(db: &DbPool, args: ClientInsertArgs) -> anyhow::Result<String> {
        let ClientInsertArgs {
            pid,
            name,
            key,
            max_bucket_size,
        } = args;

        sqlx::query("INSERT INTO clients(pid, key, name, max_bucket_size) VALUES ($1, $2, $3, $4)")
            .bind(&pid)
            .bind(key)
            .bind(name)
            .bind(max_bucket_size)
            .execute(db)
            .await?;

        Ok(pid)
    }

    pub async fn get(db: &DbPool, pid: &str) -> anyhow::Result<Client> {
        let data = sqlx::query_as("SELECT * FROM clients WHERE pid = $1 LIMIT 1")
            .bind(pid)
            .fetch_one(db)
            .await?;
        Ok(data)
    }

    pub async fn all(db: &DbPool) -> anyhow::Result<Vec<Client>> {
        let data = sqlx::query_as("SELECT * FROM clients")
            .fetch_all(db)
            .await?;
        Ok(data)
    }

    pub async fn id_by_key(db: &DbPool, key: &str) -> anyhow::Result<i32> {
        let id = sqlx::query_scalar("SELECT id FROM clients WHERE key = $1 LIMIT 1")
            .bind(key)
            .fetch_one(db)
            .await?;
        Ok(id)
    }

    pub async fn update_key(db: &DbPool, id: &str) -> anyhow::Result<String> {
        let key = Self::generate_nano();
        sqlx::query("UPDATE clients SET key = $1 WHERE pid = $2")
            .bind(&key)
            .bind(id)
            .execute(db)
            .await?;

        Ok(key)
    }

    pub fn generate_nano() -> String {
        nanoid!(10, &nanoid::alphabet::SAFE)
    }
}

#[derive(Serialize)]
pub struct ClientInsertArgs {
    pub pid: String,
    pub name: String,
    pub key: String,
    pub max_bucket_size: Option<f64>,
}

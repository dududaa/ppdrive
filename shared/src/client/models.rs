use crate::db::Database;
use crate::{AssetOwner, SqlSafe, generate_nano_id, sql_safe};
use serde::Serialize;
use sqlx::FromRow;
use time::OffsetDateTime;

#[derive(FromRow)]
pub struct Client {
    pid: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: String,
}

impl Client {
    pub async fn create(db: &Database, args: ClientInsertArgs) -> anyhow::Result<String> {
        let ClientInsertArgs {
            pid,
            name,
            key,
            max_bucket_size,
        } = args;
        let now =
            OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;

        let mut placeholders = Vec::with_capacity(5);
        for idx in 1..6 {
            placeholders.push(db.placeholder(idx))
        }

        let placeholders = placeholders.join(",");
        let query = sql_safe!(
            "INSERT INTO clients(pid, key, name, max_bucket_size, created_at) VALUES ({placeholders})"
        );

        sqlx::query(query)
            .bind(&pid)
            .bind(key)
            .bind(name)
            .bind(max_bucket_size)
            .bind(now)
            .execute(&**db)
            .await?;

        let query = sql_safe!("SELECT id FROM clients WHERE pid = {}", db.placeholder(1));
        let owner_id: i32 = sqlx::query_scalar(query)
            .bind(&pid)
            .fetch_one(&**db)
            .await?;

        let query = sql_safe!(
            "INSERT INTO asset_owner (name, owner_id) VALUES ({}, {})",
            db.placeholder(1),
            db.placeholder(2)
        );

        sqlx::query(query)
            .bind(i16::from(AssetOwner::Client))
            .bind(owner_id)
            .execute(&**db)
            .await?;

        Ok(pid)
    }

    pub async fn get_claims_data(db: &Database, id: &i32) -> anyhow::Result<(String, String)> {
        let query = sql_safe!(
            "SELECT pid, key FROM clients WHERE id = {} LIMIT 1",
            db.placeholder(1)
        );

        let data = sqlx::query_as(query).bind(id).fetch_one(&**db).await?;
        Ok(data)
    }

    pub async fn get(db: &Database, pid: &str) -> anyhow::Result<Client> {
        let query = sql_safe!(
            "SELECT * FROM clients WHERE pid = {} LIMIT 1",
            db.placeholder(1)
        );
        let data = sqlx::query_as(query).bind(pid).fetch_one(&**db).await?;

        Ok(data)
    }

    pub async fn all(db: &Database) -> anyhow::Result<Vec<Client>> {
        let data = sqlx::query_as("SELECT * FROM clients")
            .fetch_all(&**db)
            .await?;

        Ok(data)
    }

    pub async fn get_key(db: &Database, pid: &str) -> anyhow::Result<String> {
        let query = sql_safe!(
            "SELECT key FROM clients WHERE pid = {} LIMIT 1",
            db.placeholder(1)
        );
        let key = sqlx::query_scalar(query).bind(pid).fetch_one(&**db).await?;

        Ok(key)
    }

    pub async fn id_by_key(db: &Database, key: &str) -> anyhow::Result<i32> {
        let query = sql_safe!(
            "SELECT id FROM clients WHERE key = {} LIMIT 1",
            db.placeholder(1)
        );

        let id = sqlx::query_scalar(query).bind(key).fetch_one(&**db).await?;
        Ok(id)
    }

    pub async fn update_key(db: &Database, id: &str) -> anyhow::Result<String> {
        let key = Self::generate_nano();
        let query = sql_safe!(
            "UPDATE clients SET key = {} WHERE pid = {}",
            db.placeholder(1),
            db.placeholder(2)
        );

        sqlx::query(query)
            .bind(&key)
            .bind(id)
            .execute(&**db)
            .await?;

        Ok(key)
    }

    pub fn generate_nano() -> String {
        generate_nano_id(32)
    }
}

#[derive(Serialize)]
pub struct ClientInsertArgs {
    pub pid: String,
    pub name: String,
    pub key: String,
    pub max_bucket_size: Option<f64>,
}

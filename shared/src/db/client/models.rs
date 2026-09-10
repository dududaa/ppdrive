//! Client database model and insert helpers.

use crate::db::Database;
use crate::utils::{AssetOwnerName, instance_as_string};
use crate::tools::secrets::AppSecrets;
use crate::{generate_nano_id, sql_safe};
use serde::Serialize;
use sqlx::FromRow;

#[derive(FromRow)]
#[allow(dead_code)]
pub struct Client {
    pid: String,
    name: String,
    created_at: String,
}

impl Client {
    pub async fn create(db: &Database, args: ClientInsertArgs) -> anyhow::Result<String> {
        let ClientInsertArgs { pid, name, encrypted_key, key_nonce, key_hash } = args;

        let now = instance_as_string()?;
        let mut placeholders = Vec::with_capacity(6);
        for idx in 1..7 {
            placeholders.push(db.placeholder(idx))
        }

        let placeholders = placeholders.join(",");
        let query = sql_safe!(
            "INSERT INTO clients(pid, encrypted_key, key_nonce, key_hash, name, created_at) VALUES ({placeholders})"
        );

        sqlx::query(query)
            .bind(&pid)
            .bind(&encrypted_key)
            .bind(&key_nonce)
            .bind(&key_hash)
            .bind(name)
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
            .bind(i16::from(AssetOwnerName::Client))
            .bind(owner_id)
            .execute(&**db)
            .await?;

        Ok(pid)
    }

    pub async fn get_claims_data(db: &Database, id: &i32, secrets: &AppSecrets) -> anyhow::Result<(String, String)> {
        let query = sql_safe!(
            "SELECT pid, encrypted_key, key_nonce FROM clients WHERE id = {} LIMIT 1",
            db.placeholder(1)
        );

        let row: (String, String, Vec<u8>) = sqlx::query_as(query)
            .bind(id)
            .fetch_one(&**db)
            .await?;

        let plaintext_key = super::decrypt_key(secrets, &row.1, &row.2)?;
        Ok((row.0, plaintext_key))
    }

    pub async fn get(db: &Database, pid: &str) -> anyhow::Result<Client> {
        let query = sql_safe!(
            "SELECT pid, name, created_at FROM clients WHERE pid = {} LIMIT 1",
            db.placeholder(1)
        );
        let data = sqlx::query_as(query).bind(pid).fetch_one(&**db).await?;

        Ok(data)
    }

    pub async fn all(db: &Database) -> anyhow::Result<Vec<Client>> {
        let data = sqlx::query_as("SELECT pid, name, created_at FROM clients")
            .fetch_all(&**db)
            .await?;

        Ok(data)
    }

    /// Retrieve and decrypt the client's plaintext key.
    pub async fn get_key_encrypted(db: &Database, pid: &str, secrets: &AppSecrets) -> anyhow::Result<String> {
        let query = sql_safe!(
            "SELECT encrypted_key, key_nonce FROM clients WHERE pid = {} LIMIT 1",
            db.placeholder(1)
        );
        let row: (String, Vec<u8>) = sqlx::query_as(query).bind(pid).fetch_one(&**db).await?;

        super::decrypt_key(secrets, &row.0, &row.1)
    }

    /// Look up client id by key hash (O(1) index lookup).
    pub async fn id_by_key_hash(db: &Database, key_hash: &str) -> anyhow::Result<i32> {
        let query = sql_safe!(
            "SELECT id FROM clients WHERE key_hash = {} LIMIT 1",
            db.placeholder(1)
        );
        let id = sqlx::query_scalar(query).bind(key_hash).fetch_one(&**db).await?;
        Ok(id)
    }

    /// Update client's encrypted key, nonce, and hash.
    pub async fn update_key(
        db: &Database,
        pid: &str,
        encrypted_key: &str,
        key_nonce: &[u8],
        key_hash: &str,
    ) -> anyhow::Result<()> {
        let query = sql_safe!(
            "UPDATE clients SET encrypted_key = {}, key_nonce = {}, key_hash = {} WHERE pid = {}",
            db.placeholder(1),
            db.placeholder(2),
            db.placeholder(3),
            db.placeholder(4)
        );

        sqlx::query(query)
            .bind(encrypted_key)
            .bind(key_nonce)
            .bind(key_hash)
            .bind(pid)
            .execute(&**db)
            .await?;

        Ok(())
    }

    pub fn generate_nano() -> String {
        generate_nano_id(32)
    }
}

/// Insert arguments for a new client row.
#[derive(Serialize)]
pub struct ClientInsertArgs {
    pub pid: String,
    pub name: String,
    pub encrypted_key: String,
    pub key_nonce: Vec<u8>,
    pub key_hash: String,
}

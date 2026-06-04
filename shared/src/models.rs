use crate::DbPool;
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use serde::Serialize;
use sqlx::{FromRow, Sqlite, SqlitePool};
use sqlx_qb::prelude::*;

#[derive(Model, FromRow)]
#[model(table_name = "clients")]
pub struct Client {
    pid: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: DateTime<Utc>,
}

const TABLE_NAME: &'static str = <Client as Model<Sqlite, &SqlitePool>>::TABLE_NAME;

impl Client {
    pub async fn create(db: &DbPool, args: ClientInsertArgs) -> anyhow::Result<String> {
        let mut qb = QB::new(db).with_table_name(TABLE_NAME);
        let id = qb.insert_returns(&args, "pid").await?;
        Ok(id)
    }

    pub async fn get(db: &DbPool, pid: &str) -> anyhow::Result<Client> {
        let modifiers = Modifiers::new()
            .with_filter(("pid", pid))
            .with_limit(1);

        let mut qb = QB::new(db);
        qb.set_modifiers(&modifiers);

        let data = qb
            .select()
            .await?;

        Ok(data)
    }

    pub async fn all(db: &DbPool) -> anyhow::Result<Vec<Client>> {
        let mut qb = QB::new(db);
        let data = qb
            .select_all()
            .await?;

        Ok(data)
    }

    pub async fn id_by_key(db: &DbPool, key: &str) -> anyhow::Result<String> {
        let modifiers = Modifiers::new()
            .with_filter(("key", key))
            .with_limit(1);

        let mut qb = QB::new(db).with_table_name(TABLE_NAME);
        qb.set_modifiers(&modifiers);

        let id = qb.select_scalar("pid").await?;
        Ok(id)
    }

    pub async fn update_key(db: &DbPool, id: &str) -> anyhow::Result<String> {
        let modifiers = Modifiers::new().with_filter(("pid", id));
        let mut qb = QB::new(db).with_table_name(TABLE_NAME);
        qb.set_modifiers(&modifiers);

        let key = Self::generate_nano();
        let map = query_map! { "key": &key };
        qb.update(&map).await?;

        Ok(key)
    }

    pub fn generate_nano() -> String {
        nanoid!(10, &nanoid::alphabet::SAFE)
    }
}

#[derive(FromRow)]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    key: String,
    pub created_at: String,
    pub max_bucket_size: Option<f64>,
}

impl ClientInfo {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Serialize, ModelInsert)]
#[model(insert_returns = "String")]
pub struct ClientInsertArgs {
    pub pid: String,
    pub name: String,
    pub key: String,
    pub max_bucket_size: Option<f64>,
}

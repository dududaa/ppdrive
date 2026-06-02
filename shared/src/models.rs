use crate::PP;
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use serde::Serialize;
use sqlx::FromRow;
use sqlx_qb::prelude::*;

#[derive(Model, FromRow)]
#[model(table_name = "clients")]
#[model(insert_returns = "String")]
pub struct Client {
    id: i32,
    pid: String,
    key: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: DateTime<Utc>,
}

impl Client {
    pub async fn create(db: &PP, args: ClientInsertArgs<'_>) -> anyhow::Result<String> {
        let mut qb = QB::new(db);
        let id = qb.insert_returns::<Client, _>(&args, "id").await?;
        Ok(id)
    }

    pub async fn get_info(db: &PP, key: &str) -> anyhow::Result<ClientInfo> {
        let modifiers = QueryModifiers::new()
            .with_filter(("key", key))
            .with_limit(1);
        let mut qb = QB::new(db);
        qb.set_modifiers(&modifiers);

        let data = qb
            .select_fields::<Client, _>([
                "pid AS id",
                "name",
                "max_bucket_size",
                "key",
                "created_at",
            ])
            .await?;

        Ok(data)
    }

    pub async fn all(db: &PP) -> anyhow::Result<Vec<ClientInfo>> {
        let mut qb = QB::new(db);
        let data = qb
            .select_fields_all::<Client, _>([
                "pid AS id",
                "name",
                "max_bucket_size",
                "key",
                "created_at",
            ])
            .await?;

        Ok(data)
    }

    pub async fn update_key(db: &PP, id: &str) -> anyhow::Result<String> {
        let modifiers = QueryModifiers::new().with_filter(("pid", id)).with_limit(1);
        let mut qb = QB::new(db);
        qb.set_modifiers(&modifiers);

        let key = Self::generate_key();
        let map = query_map! { "key": &key };
        qb.update::<Client, _>(&map).await?;

        Ok(key)
    }

    pub fn generate_key() -> String {
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

#[derive(Serialize)]
pub struct ClientInsertArgs<'a> {
    pub name: &'a str,
    pub key: &'a str,
    pub max_bucket_size: Option<f64>,
}

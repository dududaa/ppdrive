use modeller::prelude::*;
use ppd_shared::opts::ClientInfo;
use rbatis::{RBatis, crud, impl_select, rbdc::DateTime};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::DBResult;

use super::check_model;

#[derive(Serialize, Deserialize, Modeller)]
pub struct Clients {
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,

    /// keep away from public API
    #[modeller(unique)]
    key: String,

    #[modeller(length = 120)]
    name: String,

    max_bucket_size: Option<u64>,

    created_at: DateTime,
}

crud!(Clients {});
impl_select!(Clients { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Clients {
    pub async fn get(rb: &RBatis, pid: &str) -> DBResult<Self> {
        let client = Clients::get_by_key(rb, "pid", pid).await?;
        check_model(client, "client not found")
    }

    /// retrieve client using key column
    pub async fn get_with_key(rb: &RBatis, key: &str) -> DBResult<Self> {
        let client = Clients::get_by_key(rb, "key", key).await?;
        check_model(client, "client not found")
    }

    pub async fn create(
        rb: &RBatis,
        key: String,
        name: String,
        max_bucket_size: Option<u64>,
    ) -> DBResult<String> {
        let pid = Uuid::new_v4().to_string();
        let value = Clients {
            id: None,
            pid: pid.to_string(),
            key,
            name,
            max_bucket_size,
            created_at: DateTime::now(),
        };

        Clients::insert(rb, &value).await?;
        Ok(pid)
    }

    pub async fn update_key(&mut self, db: &RBatis) -> DBResult<()> {
        self.key = Self::new_key();
        Clients::update_by_map(db, self, value! { "key": &self.key() }).await?;

        Ok(())
    }

    pub fn new_key() -> String {
        Uuid::new_v4().to_string()
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn id(&self) -> u64 {
        self.id.unwrap_or_default()
    }

    pub fn pid(&self) -> &str {
        &self.pid
    }

    pub fn max_bucket_size(&self) -> &Option<u64> {
        &self.max_bucket_size
    }
}

impl From<&Clients> for ClientInfo {
    fn from(value: &Clients) -> Self {
        let Clients {
            pid,
            name,
            created_at,
            ..
        } = value;
        ClientInfo {
            id: pid.clone(),
            name: name.clone(),
            created_at: created_at.to_string(),
        }
    }
}

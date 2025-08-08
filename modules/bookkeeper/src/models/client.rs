use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};

use crate::DBResult;

use super::check_model;

#[derive(Serialize, Deserialize, Modeller)]
pub struct Clients {
    id: Option<u64>,

    /// keep away from public API
    #[modeller(unique)]
    key: String,

    #[modeller(length = 120)]
    name: String,
}

crud!(Clients {});
impl_select!(Clients { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Clients {
    pub async fn get(rb: &RBatis, key: &str) -> DBResult<Self> {
        let client = Clients::get_by_key(rb, "key", key).await?;
        check_model(client, "client not found")
    }

    pub async fn create(rb: &RBatis, key: String, name: String) -> DBResult<()> {
        let value = Clients {
            id: None,
            key,
            name,
        };

        Clients::insert(rb, &value).await?;
        Ok(())
    }

    pub async fn update_key(&mut self, db: &RBatis, value: String) -> DBResult<()> {
        self.key = value;
        Clients::update_by_map(db, &self, value! { "id": &self.id() }).await?;

        Ok(())
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn id(&self) -> u64 {
        self.id.unwrap_or_default()
    }
}

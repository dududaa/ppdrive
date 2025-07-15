use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use serde::{Deserialize, Serialize};

use crate::CoreResult;

use super::check_model;

#[derive(Serialize, Deserialize, Modeller)]
pub struct Clients {
    #[modeller(serial)]
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,

    #[modeller(length = 120)]
    name: String,
}

crud!(Clients {});
impl_select!(Clients { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Clients {
    pub async fn get(rb: &RBatis, id: &str) -> CoreResult<Self> {
        let client = Clients::get_by_key(rb, "pid", id).await?;
        check_model(client, "client not found")
    }

    pub async fn create(rb: &RBatis, pid: String, name: String) -> CoreResult<()> {
        let value = Clients {
            id: None,
            pid,
            name,
        };

        Clients::insert(rb, &value).await?;
        Ok(())
    }

    pub fn pid(&self) -> &str {
        &self.pid
    }

    pub fn id(&self) -> u64 {
        self.id.unwrap_or_default()
    }
}

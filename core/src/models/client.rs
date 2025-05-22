use rbatis::{RBatis, crud, impl_select};
use serde::{Deserialize, Serialize};

use crate::CoreResult;

use super::check_model;

#[derive(Serialize, Deserialize)]
pub struct Client {
    id: String,
    name: String,
}

crud!(Client {});
impl_select!(Client { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE #{key} = #{value}` LIMIT 1" });

impl Client {
    pub async fn get(rb: &RBatis, id: &str) -> CoreResult<Self> {
        let client = Client::get_by_key(rb, "id", id).await?;

        check_model(client, "client not found")
    }

    pub async fn create(rb: &RBatis, id: String, name: String) -> CoreResult<()> {
        let value = Client { id, name };

        Client::insert(rb, &value).await?;
        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

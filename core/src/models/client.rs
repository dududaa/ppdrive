use modeller::{define_models, modeller_parser};
use rbatis::{RBatis, crud, impl_select};
use serde::{Deserialize, Serialize};

use crate::CoreResult;

use super::check_model;

define_models! {
    #[derive(Serialize, Deserialize)]
    pub struct Clients {
        #[modeller(unique, primary)]
        id: String,

        #[modeller(length=120)]
        name: String,
    }
}

crud!(Clients {});
impl_select!(Clients { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Clients {
    pub async fn get(rb: &RBatis, id: &str) -> CoreResult<Self> {
        let client = Clients::get_by_key(rb, "id", id).await?;
        check_model(client, "client not found")
    }

    pub async fn create(rb: &RBatis, id: String, name: String) -> CoreResult<()> {
        let value = Clients { id, name };

        Clients::insert(rb, &value).await?;
        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

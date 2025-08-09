use crate::{DBResult, errors::Error};
use rbatis::RBatis;
use serde::{Deserialize, Deserializer};

pub mod asset;
pub mod bucket;
pub mod client;
pub mod mime;
pub mod permission;
pub mod user;

pub trait IntoSerializer {
    type Serializer;

    #[allow(async_fn_in_trait)]
    async fn into_serializer(self, rb: &RBatis) -> DBResult<Self::Serializer>;
}

fn check_model<M>(model: Option<M>, msg: &str) -> DBResult<M> {
    model.ok_or(Error::ExecError(rbatis::Error::E(msg.to_string())))
}

/// SQLite does not support boolean value directly. So we deserialize `i64` to boolean;
fn de_sqlite_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = i64::deserialize(deserializer)?;
    Ok(v != 0)
}

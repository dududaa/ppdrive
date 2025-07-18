use modeller::prelude::*;
use rbatis::RBatis;
use serde::{Deserialize, Deserializer};

use crate::{
    CoreResult,
    errors::{CoreError, DbError},
    models::{
        asset::Assets, bucket::Buckets, client::Clients, permission::AssetPermissions, user::Users,
    },
};

pub mod asset;
pub mod bucket;
pub mod client;
pub mod permission;
pub mod user;

pub trait IntoSerializer {
    type Serializer;

    #[allow(async_fn_in_trait)]
    async fn into_serializer(self, rb: &RBatis) -> Result<Self::Serializer, CoreError>;
}

pub(self) fn check_model<M>(model: Option<M>, msg: &str) -> Result<M, CoreError> {
    model.ok_or(CoreError::DbError(DbError::E(msg.to_string())))
}

pub async fn run_migrations(url: &str) -> CoreResult<()> {
    let config = ConfigBuilder::new().db_url(url).build();

    Assets::write_stream(&config).await?;
    Clients::write_stream(&config).await?;
    AssetPermissions::write_stream(&config).await?;
    Users::write_stream(&config).await?;
    Buckets::write_stream(&config).await?;

    run_modeller(&config).await?;
    Ok(())
}

/// SQLite does not support boolean value directly. So we deserialize `i64` to boolean;
pub fn de_sqlite_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = i64::deserialize(deserializer)?;
    Ok(v != 0)
}

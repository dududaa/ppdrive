use modeller::{config::ConfigBuilder, run_modeller};
use rbatis::RBatis;

use crate::{
    CoreResult,
    errors::{CoreError, DbError},
    models::{asset::Assets, client::Clients, permission::AssetPermissions, user::Users},
};

pub mod asset;
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

    run_modeller(&config).await?;
    Ok(())
}

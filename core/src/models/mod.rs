use modeller::run_modeller;
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

pub async fn run_migrations() -> CoreResult<()> {
    Assets::write_stream().await?;
    Clients::write_stream().await?;
    AssetPermissions::write_stream().await?;
    Users::write_stream().await?;

    run_modeller().await?;
    Ok(())
}

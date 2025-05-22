use rbatis::RBatis;

use crate::errors::{CoreError, DbError};

pub mod asset;
pub mod client;
pub mod permission;
pub mod user;

pub trait IntoSerializer {
    type Serializer;
    async fn into_serializer(self, rb: &RBatis) -> Result<Self::Serializer, CoreError>;
}

pub(self) fn check_model<M>(model: Option<M>, msg: &str) -> Result<M, CoreError> {
    model.ok_or(CoreError::DbError(DbError::E(msg.to_string())))
}

use crate::{errors::AppError, state::AppState};

pub mod asset;
pub mod client;
pub mod permission;
pub mod user;

pub trait IntoSerializer {
    type Serializer;
    async fn into_serializer(self, state: &AppState) -> Result<Self::Serializer, AppError>;
}

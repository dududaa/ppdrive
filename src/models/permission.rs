use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::{
    errors::AppError,
    state::AppState,
    utils::sqlx::sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
};

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum Permission {
    Create,
    Read,
    Update,
    Delete,
}

impl From<Permission> for i16 {
    fn from(value: Permission) -> Self {
        match value {
            Permission::Create => 0,
            Permission::Read => 1,
            Permission::Update => 2,
            Permission::Delete => 3,
        }
    }
}

impl TryFrom<i16> for Permission {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Permission::Create),
            1 => Ok(Permission::Read),
            2 => Ok(Permission::Update),
            3 => Ok(Permission::Delete),
            _ => Err(AppError::ParsingError(format!(
                "'{value}' is invalid permission."
            ))),
        }
    }
}

#[derive(FromRow)]
pub struct AssetPermission {
    user_id: i32,
    asset_id: i32,

    #[sqlx(try_from = "i16")]
    permission: Permission,
}

impl AssetPermission {
    pub async fn create(
        state: &AppState,
        asset_id: &i32,
        fellow_id: &i32,
        permission: Permission,
    ) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let values = SqlxValues(3, 1).to_query(bn);
        let query =
            format!("INSERT INTO asset_permissions (user_id, asset_id, permission) {values}");

        let permission = i16::from(permission);
        sqlx::query(&query)
            .bind(fellow_id)
            .bind(asset_id)
            .bind(permission)
            .execute(&conn)
            .await?;

        Ok(())
    }

    pub async fn delete_for_asset(state: &AppState, asset_id: &i32) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("asset_id", 1).to_query(bn);
        let query = format!("DELETE FROM asset_permissions WHERE {filters}");

        sqlx::query(&query).bind(asset_id).execute(&conn).await?;

        Ok(())
    }

    pub async fn delete_for_user(state: &AppState, user_id: &i32) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("user_id", 1).to_query(bn);
        let query = format!("DELETE FROM asset_permissions WHERE {filters}");

        sqlx::query(&query).bind(user_id).execute(&conn).await?;

        Ok(())
    }

    pub async fn check(
        state: &AppState,
        user_id: &i32,
        asset_id: &i32,
        permission: Permission,
    ) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();
        let permission = i16::from(permission);

        let filters = SqlxFilters::new("user_id", 1)
            .and("asset_id")
            .and("permission")
            .to_query(bn);

        let query = format!("SELECT * FROM asset_permissions WHERE {filters}");
        sqlx::query_as::<_, AssetPermission>(&query)
            .bind(user_id)
            .bind(asset_id)
            .bind(permission)
            .fetch_one(&conn)
            .await?;

        Ok(())
    }
}

use rbatis::{RBatis, crud, impl_select};
use serde::{Deserialize, Serialize};

use crate::errors::CoreError;

use crate::CoreResult;

use super::check_model;
#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum Permission {
    Create,
    Read,
    Update,
    Delete,
}

impl From<Permission> for u8 {
    fn from(value: Permission) -> Self {
        match value {
            Permission::Create => 0,
            Permission::Read => 1,
            Permission::Update => 2,
            Permission::Delete => 3,
        }
    }
}

impl TryFrom<u8> for Permission {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Permission::Create),
            1 => Ok(Permission::Read),
            2 => Ok(Permission::Update),
            3 => Ok(Permission::Delete),
            _ => Err(CoreError::ParseError(format!(
                "'{value}' is invalid permission."
            ))),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AssetPermissions {
    user_id: u64,
    asset_id: u64,
    permission: u8,
}

crud!(AssetPermissions {});
impl_select!(AssetPermissions{ check(user_id: &u64, asset_id: &u64, permission: &u8) -> Option => "`WHERE user_id = #{user_id} AND asset_id = #{user_id} AND permission = #{permission}`" });

impl AssetPermissions {
    pub async fn create(
        rb: &RBatis,
        asset_id: &u64,
        fellow_id: &u64,
        permission: Permission,
    ) -> CoreResult<()> {
        let value = AssetPermissions {
            asset_id: *asset_id,
            user_id: *fellow_id,
            permission: permission.into(),
        };

        AssetPermissions::insert(rb, &value).await?;

        Ok(())
    }

    pub async fn delete_for_asset(rb: &RBatis, asset_id: &u64) -> CoreResult<()> {
        AssetPermissions::delete_by_column(rb, "asset_id", asset_id).await?;
        Ok(())
    }

    pub async fn delete_for_user(rb: &RBatis, user_id: &u64) -> CoreResult<()> {
        AssetPermissions::delete_by_column(rb, "user_id", user_id).await?;
        Ok(())
    }

    pub async fn exists(
        rb: &RBatis,
        user_id: &u64,
        asset_id: &u64,
        permission: Permission,
    ) -> CoreResult<()> {
        let pd = u8::from(permission);
        let perm = AssetPermissions::check(rb, user_id, asset_id, &pd).await?;

        check_model(perm, "permission does not exist")?;
        Ok(())
    }
}

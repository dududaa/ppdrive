use crate::{DBResult, Error};

use rbatis::RBatis;

use crate::models::asset::{AssetType, Assets};

pub struct ValidatePathDetails<'a> {
    pub path: &'a str,
    pub custom_path: &'a Option<String>,
    pub ty: &'a AssetType,
}

/// Validate whether `asset_path` or `custom_path` is already used by another asset
pub async fn validate_asset_paths(db: &RBatis, asset: ValidatePathDetails<'_>) -> DBResult<()> {
    let ValidatePathDetails {
        path,
        ty,
        custom_path,
        ..
    } = asset;

    let path = custom_path.clone().unwrap_or(path.to_string());
    let asset = Assets::get_by_path(db, &path, ty).await.ok();

    if asset.is_some() {
        let field = if custom_path.is_some() {
            "custom_path"
        } else {
            "asset_path"
        };

        Err(Error::PermissionError(format!(
            "provided {field} already used by another asset."
        )))
    } else {
        Ok(())
    }
}

use std::path::PathBuf;

use ppd_bk::models::asset::AssetType;

use crate::FsResult;

pub async fn create_or_update(
    asset_type: &AssetType,
    asset_path: &str,
    tmp: &Option<PathBuf>,
) -> FsResult<()> {
    match asset_type {
        AssetType::File => {
            if let Some(tmp) = tmp {
                tokio::fs::rename(tmp, asset_path).await?;
            }
        }
        AssetType::Folder => tokio::fs::create_dir(asset_path).await?,
    }

    Ok(())
}

pub async fn delete_asset(asset_type: &AssetType, asset_path: &str) -> FsResult<()> {
    match asset_type {
        AssetType::File => tokio::fs::remove_file(asset_path).await?,
        AssetType::Folder => tokio::fs::remove_dir_all(asset_path).await?,
    }

    Ok(())
}

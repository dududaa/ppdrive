use std::path::{Path, PathBuf};

use ppd_bk::RBatis;
use ppd_bk::models::asset::{AssetType, Assets, NewAsset, UpdateAssetValues};
use ppd_bk::validators::{ValidatePathDetails, validate_asset_paths};
use ppd_shared::tracing;

use crate::errors::Error;
use crate::utils::move_file;
use crate::{FsResult, opts::CreateAssetOptions};

pub async fn create_or_update(
    db: &RBatis,
    user_id: &u64,
    bucket_id: &u64,
    opts: &CreateAssetOptions,
    tmp: &Option<PathBuf>,
    dest: &Path,
) -> FsResult<()> {
    let CreateAssetOptions {
        asset_path,
        asset_type,
        public,
        custom_path,
        create_parents,
        sharing,
        update_asset_path,
        ..
    } = opts;

    // validate paths
    let vd = ValidatePathDetails {
        path: asset_path,
        ty: asset_type,
        custom_path,
    };

    if let Err(err) = validate_asset_paths(db, vd).await {
        if let Some(tmp) = tmp {
            if let Err(err) = tokio::fs::remove_file(tmp).await {
                tracing::error!("removing tmp file after asset validation: {err}")
            }
        }

        return Err(err.into());
    }

    // create parents if required
    if create_parents.unwrap_or(true) {
        create_asset_parents(db, dest, user_id, bucket_id, public).await?;
    }

    match asset_type {
        AssetType::File => {
            if let Some(tmp) = tmp {
                move_file(tmp, &dest).await?;
            }
        }
        AssetType::Folder => tokio::fs::create_dir(&dest).await?,
    }

    // if path already exists, update it. Else, create.
    let path = custom_path.clone().unwrap_or(asset_path.to_string());
    let public = public.unwrap_or_default();

    let asset: Result<Assets, Error> = match Assets::get_by_path(db, &path, asset_type).await {
        Ok(mut exists) => {
            if exists.user_id() != user_id {
                if let Some(tmp) = tmp {
                    if let Err(err) = tokio::fs::remove_file(tmp).await {
                        tracing::warn!("{err}")
                    }
                }

                return Err(Error::PermissionError(
                    "you do not have permission to update this resource.".to_string(),
                ));
            }

            let asset_path = update_asset_path.clone().unwrap_or(asset_path.to_string());
            let values = UpdateAssetValues {
                asset_path,
                custom_path: custom_path.clone(),
                public,
            };

            exists.update(db, values).await?;
            Ok(exists)
        }
        Err(_) => {
            let value = NewAsset {
                user_id: *user_id,
                public,
                asset_path: asset_path.to_string(),
                custom_path: custom_path.clone(),
                asset_type: u8::from(asset_type),
                bucket_id: *bucket_id,
            };

            Assets::create(db, value).await?;
            let asset = Assets::get_by_path(db, asset_path, asset_type).await?;

            Ok(asset)
        }
    };

    // share asset with collaborators
    let asset = asset?;
    if public {
        if let Some(sharing) = sharing {
            if !sharing.is_empty() {
                asset.share(db, sharing).await?;
            }
        }
    }

    Ok(())
}

/// create asset's parents (including their records) if they don't exist.
async fn create_asset_parents(
    db: &RBatis,
    path: &Path,
    user_id: &u64,
    bucket_id: &u64,
    is_public: &Option<bool>,
) -> FsResult<()> {
    let parent = path.parent();

    if let Some(parent) = parent {
        let parents: Vec<&str> = parent.ancestors().filter_map(|p| p.to_str()).collect();
        let paths: Vec<&&str> = parents
            .iter()
            .rev()
            .filter(|p| !p.is_empty())
            .filter(|p| p != &&"/")
            .collect();

        if let Some(first) = paths.first() {
            if first.starts_with("/") {
                return Err(Error::ServerError(
                    "asset path cannot start with an '/'".to_string(),
                ));
            }
        }

        let folder_type = u8::from(&AssetType::Folder);
        let mut assets = Vec::with_capacity(paths.len());

        for path in &paths {
            // check if parent folders
            if let Ok(exist) = Assets::get_by_path(db, path, &AssetType::Folder).await {
                if exist.user_id() != user_id {
                    let msg = format!(
                        "you're attempting to create a folder at \"{path}\" which already belongs to someone else."
                    );
                    tracing::error!(msg);
                    return Err(Error::ServerError(msg));
                } else {
                    tracing::warn!("path {path} already exists. skipping... ");
                    continue;
                }
            }

            // build query values
            let asset = NewAsset {
                user_id: *user_id,
                asset_path: path.to_string(),
                custom_path: None,
                asset_type: folder_type,
                public: is_public.unwrap_or(false),
                bucket_id: *bucket_id,
            };

            assets.push(asset);
        }

        if !assets.is_empty() {
            Assets::insert_group(db, assets).await?;
        }

        tokio::fs::create_dir_all(parent).await?;
    }

    Ok(())
}

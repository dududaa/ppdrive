use std::path::{Path, PathBuf};

use ppd_bk::RBatis;
use ppd_bk::models::asset::{AssetType, Assets, NewAsset, UpdateAssetValues};
use ppd_bk::models::bucket::Buckets;
use ppd_bk::validators::{ValidatePathDetails, validate_asset_paths};
use ppd_shared::tools::mb_to_bytes;

use crate::errors::Error;
use crate::utils::{create_asset_parents, get_bucket_size};
use crate::{FsResult, opts::CreateAssetOptions};

/// create or update an asset
pub async fn create_or_update_asset(
    db: &RBatis,
    user_id: &u64,
    opts: &CreateAssetOptions,
    tmp: &Option<PathBuf>,
    filesize: &Option<u64>,
) -> FsResult<()> {
    let CreateAssetOptions {
        asset_path,
        asset_type,
        public,
        custom_path,
        create_parents,
        sharing,
        update_asset_path,
        bucket,
    } = opts;

    // retrieve bucket and validate bucket ownership
    let bucket = Buckets::get_by_pid(db, bucket).await?;
    if !bucket.validate_write(user_id) {
        return Err(Error::PermissionError(
            "you have not permission to write to this bucket".to_string(),
        ));
    }

    // extract destination path
    let partition = bucket.partition().as_deref();
    let dest = partition.map_or(asset_path.to_string(), |rf| format!("{rf}/{asset_path}"));
    let dest = Path::new(&dest);

    // validate file mimetype and check bucket size limit
    if let Some(tmp_file) = tmp {
        let mime_type = mime_guess::from_path(tmp_file).first_or_octet_stream();
        let mime = mime_type.to_string();
        bucket.validate_mime(db, &mime).await?;

        if let (Some(filesize), Some(max_size)) = (filesize, bucket.partition_size()) {
            let current_size = get_bucket_size(&bucket).await?;
            let total_size = current_size + filesize;
            if total_size > mb_to_bytes(*max_size) as u64 {
                tokio::fs::remove_file(tmp_file).await?;

                return Err(Error::ServerError("bucket size exceeded.".to_string()));
            }
        }
    }

    // validate paths
    let vd = ValidatePathDetails {
        path: asset_path,
        ty: asset_type,
        custom_path,
    };

    validate_asset_paths(db, vd).await?;

    // create parents if required
    if create_parents.unwrap_or(true) {
        create_asset_parents(db, dest, user_id, &bucket.id(), public).await?;
    }

    match asset_type {
        AssetType::File => {
            if let Some(tmp) = tmp {
                #[cfg(target_os = "linux")]
                {
                    tokio::fs::copy(tmp, &dest).await?;
                    tokio::fs::remove_file(tmp).await?;
                }

                #[cfg(not(target_os = "linux"))]
                tokio::fs::rename(tmp, &dest).await?;
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
                bucket_id: bucket.id(),
            };

            Assets::create(db, value).await?;
            let asset = Assets::get_by_path(db, asset_path, asset_type).await?;

            Ok(asset)
        }
    };

    // share asset with collaborators
    let asset = asset?;
    if public
        && let Some(sharing) = sharing
        && !sharing.is_empty()
    {
        asset.share(db, sharing).await?;
    }

    Ok(())
}

/// removes an asset and associated records. if asset is a folder, this will remove all its content as well
pub async fn delete_asset(db: &RBatis, path: &str, asset_type: &AssetType) -> FsResult<()> {
    // delete asset records
    let asset = Assets::get_by_path(db, path, asset_type).await?;
    asset.delete(db).await?;

    // delete asset's children records
    if let AssetType::Folder = asset_type {
        let mut entries = tokio::fs::read_dir(asset.path()).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let child_type = if path.is_file() {
                AssetType::File
            } else {
                AssetType::Folder
            };

            if let Some(path) = path.to_str()
                && let Ok(child) = Assets::get_by_path(db, path, &child_type).await
            {
                child.delete(db).await?;
            }
        }
    }

    // delete asset
    match asset_type {
        AssetType::File => tokio::fs::remove_file(path).await?,
        AssetType::Folder => tokio::fs::remove_dir_all(path).await?,
    }

    Ok(())
}

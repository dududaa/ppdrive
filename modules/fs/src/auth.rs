use std::path::{Path, PathBuf};

use ppd_bk::RBatis;
use ppd_bk::models::asset::{AssetType, Assets, NewAsset, UpdateAssetValues};
use ppd_bk::models::bucket::Buckets;
use ppd_bk::validators::{ValidatePathDetails, validate_asset_paths};
use ppd_shared::tools::{SECRETS_FILENAME, mb_to_bytes};

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
) -> FsResult<String> {
    let CreateAssetOptions {
        asset_path,
        asset_type,
        public,
        slug: custom_path,
        create_parents,
        sharing,
        bucket,
        overwrite: replace,
    } = opts;

    if tmp.is_none()
        && let AssetType::File = asset_type
    {
        return Err(Error::ServerError(
            "file object must be included in the request".to_string(),
        ));
    }

    if opts.asset_path.is_empty() {
        return Err(Error::ServerError(
            "asset_path field is required".to_string(),
        ));
    }

    if opts.asset_path == SECRETS_FILENAME {
        return Err(Error::ServerError(
            "asset_path '{SECRET_FILE}' is reserved. please choose another path.".to_string(),
        ));
    }

    // retrieve bucket and validate bucket ownership
    let bucket = Buckets::get_by_pid(db, bucket).await?;
    if !bucket.validate_write(user_id) {
        return Err(Error::PermissionError(
            "you have not permission to write to this bucket".to_string(),
        ));
    }

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

    // extract destination path
    let partition = bucket.partition().as_deref();
    let dest = partition.map_or(asset_path.to_string(), |rf| format!("{rf}/{asset_path}"));

    // validate paths
    let vd = ValidatePathDetails {
        path: &dest,
        ty: asset_type,
        custom_path,
    };

    validate_asset_paths(db, vd).await?;

    let public = public.unwrap_or(bucket.public());
    let path = custom_path.clone().unwrap_or(dest.clone());
    let slug = urlencoding::encode(&path).to_string();

    // if path already exists, update it. Else, create.
    let asset: Result<Assets, Error> = match Assets::get_by_slug(db, &slug, asset_type).await {
        Ok(mut exists) => {
            if exists.user_id() != user_id {
                return Err(Error::PermissionError(
                    "you do not have permission to update this resource.".to_string(),
                ));
            }

            if !replace.unwrap_or_default() {
                return Err(Error::PermissionError("asset already exists. if you intend to overwrite exisiting asset, set the `overwrite` option to `true`.".to_string()));
            }

            let values = UpdateAssetValues {
                asset_path: dest,
                slug,
                public,
            };

            exists.update(db, values).await?;
            Ok(exists)
        }
        Err(_) => {
            let value = NewAsset {
                user_id: *user_id,
                public,
                asset_path: dest,
                slug,
                asset_type: u8::from(asset_type),
                bucket_id: bucket.id(),
            };

            Assets::create(db, value).await?;
            let asset = Assets::get_by_slug(db, asset_path, asset_type).await?;

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

    // create parents if required
    if create_parents.unwrap_or(true) {
        let path = Path::new(asset.path());
        create_asset_parents(db, path, user_id, &bucket.id(), public).await?;
    }

    // save asset record
    let dest = asset.path();
    match asset_type {
        AssetType::File => {
            if let Some(tmp) = tmp {
                #[cfg(target_os = "linux")]
                {
                    tokio::fs::copy(tmp, dest).await?;
                    tokio::fs::remove_file(tmp).await?;
                }

                #[cfg(not(target_os = "linux"))]
                tokio::fs::rename(tmp, dest).await?;
            }
        }
        AssetType::Folder => tokio::fs::create_dir(dest).await?,
    }

    Ok(asset.path().to_string())
}

/// removes an asset and associated records. if asset is a folder, this will remove all its content as well
pub async fn delete_asset(
    db: &RBatis,
    user_id: &u64,
    path: &str,
    asset_type: &AssetType,
) -> FsResult<()> {
    let slug = urlencoding::decode(&path).map_err(|err| Error::ServerError(err.to_string()))?;
    let asset = Assets::get_by_slug(db, &slug, asset_type).await?;

    if asset.user_id() != user_id {
        return Err(Error::PermissionError(
            "you have no permission to delete this asset".to_string(),
        ));
    }
    
    // delete asset's children records
    asset.delete(db).await?;
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
                && let Ok(child) = Assets::get_by_slug(db, path, &child_type).await
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

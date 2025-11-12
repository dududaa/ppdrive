use std::path::Path;

use ppd_bk::{
    RBatis,
    models::{
        asset::{AssetType, Assets, NewAsset},
        bucket::Buckets,
    },
};

use crate::{FsResult, errors::Error};

/// create asset's parents (including their records) if they don't exist.
pub async fn create_asset_parents(
    db: &RBatis,
    asset_path: &Path,
    user_id: &u64,
    bucket_id: &u64,
    is_public: bool,
) -> FsResult<()> {
    let parent = asset_path.parent();

    if let Some(parent) = parent {
        let parents: Vec<&str> = parent.ancestors().filter_map(|p| p.to_str()).collect();
        let paths: Vec<&&str> = parents
            .iter()
            .rev()
            .filter(|p| !p.is_empty())
            .filter(|p| p != &&"/")
            .collect();

        if let Some(first) = paths.first()
            && first.starts_with("/")
        {
            return Err(Error::ServerError(
                "asset path cannot start with an '/'".to_string(),
            ));
        }

        let folder_type = u8::from(&AssetType::Folder);
        let mut assets = Vec::with_capacity(paths.len());

        for path in &paths {
            // check if parent folders
            if let Some(exist) = Assets::select_by_path(db, path, folder_type)
                .await
                .map_err(|err| Error::ServerError(err.to_string()))?
            {
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
            let slug = urlencoding::encode_exclude(&path, &['/']).to_string();
            let asset = NewAsset {
                user_id: *user_id,
                asset_path: path.to_string(),
                slug,
                asset_type: folder_type,
                public: is_public,
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

pub async fn get_bucket_size(bucket: &Buckets) -> FsResult<u64> {
    let mut size = 0;
    if let Some(partition) = bucket.partition() {
        let dir = Path::new(partition);
        if !dir.exists() {
            tokio::fs::create_dir_all(dir).await?;
            return Ok(size);
        }

        get_folder_size(partition, &mut size).await?;
    }

    Ok(size)
}

/// compute total size (in bytes) of a folder.
async fn get_folder_size(folder_path: &str, size: &mut u64) -> FsResult<()> {
    let path = Path::new(folder_path);

    if path.is_file() {
        return Err(Error::ServerError(
            "provided path is not a folder path".to_string(),
        ));
    }

    let mut rd = tokio::fs::read_dir(path).await?;

    while let Ok(Some(entry)) = rd.next_entry().await {
        let path = entry.path();

        if path.is_file() {
            let m = path.metadata()?;
            *size += m.len()
        } else if let Some(folder) = path.to_str() {
            Box::pin(get_folder_size(folder, size)).await?;
        }
    }

    Ok(())
}

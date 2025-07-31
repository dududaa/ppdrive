use std::path::{Path, PathBuf};

use rbatis::RBatis;
use rbs::value;

use crate::{
    CoreResult,
    errors::CoreError,
    models::{permission::AssetPermissions, user::Users},
    options::{AssetSharing, CreateAssetOptions},
};

use super::{AssetType, Assets};

pub(super) struct SaveAssetOpts<'a> {
    pub path: &'a str,
    pub is_public: &'a Option<bool>,
    pub custom_path: &'a Option<String>,
    pub user_id: &'a u64,
    pub asset_type: &'a AssetType,
}

/// Validate whether `asset_path` or `custom_path` is already used by another asset
pub(super) async fn validate_paths(
    rb: &RBatis,
    opts: &CreateAssetOptions,
    tmp: &Option<PathBuf>,
) -> Result<(), CoreError> {
    let CreateAssetOptions {
        asset_path,
        asset_type,
        custom_path,
        ..
    } = opts;
    let check_path = Assets::get_by_path(rb, asset_path, asset_type).await.ok();
    let mut check_cp = None;
    if let Some(custom_path) = custom_path {
        check_cp = Assets::get_by_path(rb, custom_path, asset_type).await.ok();
    }

    if check_path.is_some() || check_cp.is_some() {
        let field = if check_path.is_some() {
            "asset_path"
        } else {
            "custom_path"
        };

        if let Some(tmp) = tmp {
            tokio::fs::remove_file(tmp).await?;
        }

        return Err(CoreError::ServerError(format!(
            "provided {field} already used by another asset."
        )));
    }

    Ok(())
}

/// Traverses asset path, create each parent dir and their respective
/// records where they don't exist. Returns the id of the last path created.
///
///
/// Does not check if user exists. Caller is responsible for validating
/// `user_id`.
pub(super) async fn create_asset_parents(
    rb: &RBatis,
    path: &Path,
    user_id: &u64,
    bucker_id: &u64,
    is_public: &Option<bool>,
) -> CoreResult<()> {
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
                return Err(CoreError::ServerError(
                    "asset path cannot start with an '/'".to_string(),
                ));
            }
        }

        let folder_type = u8::from(&AssetType::Folder);
        let mut assets = Vec::with_capacity(paths.len());

        for path in &paths {
            // check if parent folders
            if let Ok(exist) = Assets::get_by_path(rb, path, &AssetType::Folder).await {
                if exist.user_id() != user_id {
                    let msg = format!(
                        "you're attempting to create a folder at \"{path}\" which already belongs to someone else."
                    );
                    tracing::error!(msg);
                    return Err(CoreError::ServerError(msg));
                } else {
                    tracing::warn!("path {path} already exists. skipping... ");
                    continue;
                }
            }

            // build query values
            let asset = Assets {
                id: None,
                user_id: *user_id,
                asset_path: path.to_string(),
                custom_path: None,
                asset_type: folder_type,
                public: is_public.unwrap_or(false),
                bucket_id: *bucker_id,
            };

            assets.push(asset);
        }

        if !assets.is_empty() {
            Assets::insert_batch(rb, &assets, assets.len() as u64).await?;
        }

        tokio::fs::create_dir_all(parent).await?;
    }

    Ok(())
}

pub(super) async fn share_asset(
    rb: &RBatis,
    sharing: &Vec<AssetSharing>,
    asset_id: &u64,
    user_id: &u64,
) -> CoreResult<()> {
    for opt in sharing {
        let get_fellow = Users::get_by_pid(rb, &opt.user_id).await;
        if let Err(err) = get_fellow {
            tracing::error!("error getting user to share asset with: {err}");
            continue;
        }

        let fellow = get_fellow?;
        let fellow_id = &fellow.id();
        if user_id == fellow_id {
            tracing::error!("you cannot share asset {asset_id} with it's owner");
            continue;
        }

        if opt.permissions.is_empty() {
            tracing::error!("permissions list must be specifed for a sharing option");
            continue;
        }

        for permission in &opt.permissions {
            AssetPermissions::create(rb, asset_id, fellow_id, permission.clone()).await?;
        }
    }

    Ok(())
}

/// create asset if it doesn't exist, else update it
pub(super) async fn create_or_update_asset(
    rb: &RBatis,
    bucker_id: &u64,
    opts: SaveAssetOpts<'_>,
    tmp: &Option<PathBuf>,
) -> Result<Assets, CoreError> {
    let SaveAssetOpts {
        is_public,
        custom_path,
        user_id,
        path,
        asset_type,
    } = opts;

    let public = is_public.unwrap_or(false);

    match Assets::get_by_path(rb, path, asset_type).await {
        Ok(exists) => {
            if &exists.user_id == user_id {
                let updated = Assets {
                    custom_path: custom_path.clone(),
                    public,
                    ..exists
                };

                Assets::update_by_map(
                    rb,
                    &updated,
                    value! {
                        "id": updated.id()
                    },
                )
                .await?;
                Ok(updated)
            } else {
                if let Some(tmp) = tmp {
                    tokio::fs::remove_file(tmp).await?;
                }

                Err(CoreError::PermissionError(
                    "user has no permission to update asset".to_string(),
                ))
            }
        }
        Err(_) => {
            let mut asset = Assets {
                user_id: *user_id,
                public,
                asset_path: path.to_string(),
                custom_path: custom_path.clone(),
                id: None,
                asset_type: u8::from(asset_type),
                bucket_id: *bucker_id,
            };

            Assets::insert(rb, &asset).await?;
            if let Ok(n) = Assets::get_by_path(rb, path, asset_type).await {
                asset.id = n.id
            }

            Ok(asset)
        }
    }
}

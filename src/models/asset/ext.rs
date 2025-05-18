use std::path::{Path, PathBuf};

use crate::{
    errors::AppError,
    models::{permission::AssetPermission, user::User},
    sqlx_binder,
    state::AppState,
    utils::sqlx::sqlx_utils::{SqlxFilters, SqlxSetters, SqlxValues, ToQuery},
};

use super::{Asset, AssetSharing, AssetType};

pub(super) struct SaveAssetOpts<'a> {
    pub path: &'a str,
    pub is_public: &'a Option<bool>,
    pub custom_path: &'a Option<String>,
    pub user_id: &'a i32,
    pub asset_type: &'a AssetType,
    pub parent: &'a Option<i32>,
}

pub(super) async fn save_asset(state: &AppState, opts: SaveAssetOpts<'_>) -> Result<(), AppError> {
    let SaveAssetOpts {
        path,
        is_public,
        custom_path,
        user_id,
        asset_type,
        parent,
    } = opts;

    let conn = state.db_pool().await;
    let bn = state.backend_name();
    let asset_type: i16 = asset_type.into();
    let values = SqlxValues(6, 1).to_query(bn);
    let query = format!(
        "INSERT INTO assets (asset_path, public, custom_path, user_id, asset_type, parent_id) {values}"
    );
    sqlx::query(&query)
        .bind(path)
        .bind(is_public)
        .bind(custom_path)
        .bind(user_id)
        .bind(asset_type)
        .bind(parent)
        .execute(&conn)
        .await?;

    Ok(())
}

pub(super) async fn update_asset(
    state: &AppState,
    asset_id: &i32,
    is_public: &Option<bool>,
    custom_path: &Option<String>,
    parent: &Option<i32>,
) -> Result<(), AppError> {
    let conn = state.db_pool().await;
    let bn = state.backend_name();

    let sf = SqlxSetters::new("public", 1)
        .add("custom_path")
        .add("parent_id")
        .to_query(bn);

    let is_public = is_public.unwrap_or_default();
    let ff = SqlxFilters::new("id", 4).to_query(bn)?;
    let query = format!("UPDATE assets SET {sf} WHERE {ff}");

    tracing::info!("query: {query}");

    sqlx::query(&query)
        .bind(is_public)
        .bind(custom_path)
        .bind(parent)
        .bind(asset_id)
        .execute(&conn)
        .await?;

    Ok(())
}

/// Validate whether `custom_path` is already used by another asset
pub(super) async fn validate_custom_path(
    state: &AppState,
    custom_path: &str,
    path: &str,
    asset_type: &AssetType,
    tmp: &Option<PathBuf>,
) -> Result<(), AppError> {
    let exist = Asset::get_by_path(state, custom_path, asset_type).await;
    if let Ok(exist) = exist {
        if exist.asset_path != path {
            if let Some(tmp) = tmp {
                tokio::fs::remove_file(tmp).await?;
            }

            return Err(AppError::InternalServerError(format!(
                r#"asset with custom_path "{custom_path}" already exists."#
            )));
        }
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
    state: &AppState,
    path: &Path,
    user_id: &i32,
    is_public: &Option<bool>,
) -> Result<Option<i32>, AppError> {
    let parent = path.parent();
    let mut last_parent = None;

    if let Some(parent) = parent {
        let parents: Vec<&str> = parent.ancestors().filter_map(|p| p.to_str()).collect();
        let paths: Vec<&&str> = parents
            .iter()
            .rev()
            .filter(|p| !p.is_empty())
            .filter(|p| p != &&"/")
            .collect();

        let mut parents = Vec::with_capacity(paths.len());
        parents.push(None);

        if let Some(first) = paths.first() {
            if first.starts_with("/") {
                return Err(AppError::InternalServerError(
                    "asset path cannot start with an '/'".to_string(),
                ));
            }

            let parent_path = Path::new(first).parent();
            if let Some(parent_path) = parent_path {
                if let Some(np) = parent_path.to_str() {
                    if np != "" && np != "/" {
                        let parent_asset = Asset::get_by_path(state, np, &AssetType::Folder).await;
                        let parent = parent_asset.ok().map(|asset| asset.id);

                        if let Some(first) = parents.get_mut(0) {
                            *first = Some(np)
                        }

                        last_parent = parent;
                    }
                }
            }
        }

        let bn = state.backend_name();
        let mut values = Vec::with_capacity(paths.len());

        for (index, path) in paths.iter().enumerate() {
            // check if parent folders
            if let Ok(exist) = Asset::get_by_path(state, path, &AssetType::Folder).await {
                if exist.user_id() != user_id {
                    let msg = "you're attempting to create a folder that already beolngs to someone else.";
                    tracing::error!(msg);
                    return Err(AppError::InternalServerError(msg.to_string()));
                } else {
                    tracing::warn!("path {path} already exists. skipping... ");
                    last_parent = Some(exist.id);
                    continue;
                }
            }

            // build query values
            let pub_q = bn.to_query(1);
            let user_q = bn.to_query(2);
            let asset_type_q = bn.to_query(3);
            let path_q = bn.to_query((index as u8) + 4);

            values.push(format!(
                "({}, {}, {}, {})",
                pub_q, user_q, asset_type_q, path_q
            ));

            if index < (paths.len() - 1) {
                parents.push(Some(path));
            }
        }

        if !values.is_empty() {
            // build query
            let asset_type = i16::from(&AssetType::Folder);
            let query = format!(
                "INSERT INTO assets (public, user_id, asset_type, asset_path) \nVALUES \n{}",
                values.join(",\n")
            );

            // save records
            let is_public = is_public.unwrap_or_default();
            let conn = state.db_pool().await;
            sqlx_binder!(
                conn,
                &query,
                is_public,
                user_id,
                asset_type;
                &paths
            );

            // update path parents
            for (index, path) in paths.iter().enumerate() {
                let asset = Asset::get_by_path(state, path, &AssetType::Folder).await?;
                let parent = parents.get(index);

                if let Some(parent_path) = parent {
                    let mut parent_id = None;
                    if let Some(parent_path) = parent_path {
                        let parent =
                            Asset::get_by_path(state, parent_path, &AssetType::Folder).await?;
                        parent_id = Some(parent.id().clone());
                    }

                    let setters = SqlxSetters::new("parent_id", 1).to_query(bn);
                    let filters = SqlxFilters::new("id", 2).to_query(bn)?;

                    let query = format!("UPDATE assets SET {setters} WHERE {filters}");
                    sqlx::query(&query)
                        .bind(parent_id)
                        .bind(asset.id())
                        .execute(&conn)
                        .await?;
                }

                // set last path as last parent
                if paths.last() == Some(path) {
                    last_parent = Some(asset.id)
                }
            }
        }

        tokio::fs::create_dir_all(parent).await?;
    }

    Ok(last_parent)
}

pub(super) async fn move_file(src: &Option<PathBuf>, dest: &Path) -> Result<(), AppError> {
    if let Err(err) = tokio::fs::File::create(dest).await {
        tracing::info!("unable to create destination file: {err}");
        return Err(AppError::IOError(err.to_string()));
    }

    if let Some(src) = src {
        tokio::fs::copy(&src, dest).await?;
        tokio::fs::remove_file(&src).await?;
    }

    Ok(())
}

pub(super) async fn share_asset(
    state: &AppState,
    sharing: &Vec<AssetSharing>,
    asset_id: &i32,
    user_id: &i32,
) -> Result<(), AppError> {
    for opt in sharing {
        let get_fellow = User::get_by_pid(state, &opt.user_id).await;
        if let Err(err) = get_fellow {
            tracing::error!("error getting user to share asset with: {err}");
            continue;
        }

        let fellow = get_fellow?;
        let fellow_id = fellow.id();
        if user_id == fellow_id {
            tracing::error!("you cannot share asset {asset_id} with it's owner");
            continue;
        }

        if opt.permissions.is_empty() {
            tracing::error!("permissions list must be specifed for a sharing option");
            continue;
        }

        for permission in &opt.permissions {
            AssetPermission::create(state, asset_id, fellow_id, permission.clone()).await?;
        }
    }

    Ok(())
}

pub(super) async fn create_or_update_asset(
    state: &AppState,
    opts: SaveAssetOpts<'_>,
    tmp: &Option<PathBuf>,
) -> Result<Asset, AppError> {
    let SaveAssetOpts {
        is_public,
        custom_path,
        user_id,
        path,
        asset_type,
        parent,
    } = opts;

    match Asset::get_by_path(state, path, asset_type).await {
        Ok(exists) => {
            if &exists.user_id == user_id {
                update_asset(state, &exists.id, is_public, custom_path, parent).await?;
                Ok(exists)
            } else {
                if let Some(tmp) = tmp {
                    tokio::fs::remove_file(tmp).await?;
                }

                Err(AppError::AuthorizationError(
                    "user has no permission to update asset".to_string(),
                ))
            }
        }
        Err(_) => {
            save_asset(state, opts).await?;
            let asset = Asset::get_by_path(state, path, asset_type).await?;

            Ok(asset)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{errors::AppError, main_test::pretest, models::asset::create_asset_parents};

    #[tokio::test]
    async fn test_create_parents() -> Result<(), AppError> {
        let state = pretest().await?;
        let path = Path::new("start/middle/end/filename");
        let create = create_asset_parents(&state, path, &3, &None).await;
        if let Err(err) = &create {
            println!("{err}")
        }

        assert!(create.is_ok());

        Ok(())
    }
}

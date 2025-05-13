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
}

pub(super) async fn save_asset<'a>(
    state: &AppState,
    opts: SaveAssetOpts<'a>,
) -> Result<(), AppError> {
    let SaveAssetOpts {
        path,
        is_public,
        custom_path,
        user_id,
        asset_type,
    } = opts;

    let conn = state.db_pool().await;
    let bn = state.backend_name();
    let asset_type: i16 = asset_type.into();

    let values = SqlxValues(4, 1).to_query(bn);
    let query = format!(
        "INSERT INTO assets (asset_path, public, custom_path, user_id, asset_type) {values}"
    );
    sqlx::query(&query)
        .bind(path)
        .bind(is_public)
        .bind(custom_path)
        .bind(user_id)
        .bind(asset_type)
        .execute(&conn)
        .await?;

    Ok(())
}

pub(super) async fn update_asset(
    state: &AppState,
    asset_id: &i32,
    is_public: &Option<bool>,
    custom_path: &Option<String>,
) -> Result<(), AppError> {
    let conn = state.db_pool().await;
    let bn = state.backend_name();

    let sf = SqlxSetters::new("public", 1)
        .add("custom_path")
        .to_query(bn);

    let ff = SqlxFilters::new("id", 3).to_query(bn);
    let query = format!("UPDATE assets SET {sf} WHERE {ff}");

    sqlx::query(&query)
        .bind(is_public)
        .bind(custom_path)
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
/// records where they don't exist.
///
/// Does not check if user exists. Caller is responsible for validating
/// `user_id`.
pub(super) async fn create_asset_parents(
    state: &AppState,
    path: &Path,
    user_id: &i32,
    is_public: &Option<bool>,
) -> Result<(), AppError> {
    let parent = path.parent();

    if let Some(parent) = parent {
        let parents: Vec<&str> = parent.ancestors().map(|p| p.to_str()).flatten().collect();
        let paths: Vec<&&str> = parents
            .iter()
            .rev()
            .filter(|p| !p.is_empty())
            .filter(|p| p != &&"/")
            .collect();

        if let Some(first) = paths.first() {
            if first.starts_with("/") {
                return Err(AppError::InternalServerError(
                    "asset path cannot start with an '/'".to_string(),
                ));
            }
        }

        let bn = state.backend_name();
        let mut values = Vec::with_capacity(parents.len());

        for (index, path) in paths.iter().enumerate() {
            // check if parent folders
            if let Ok(exist) = Asset::get_by_path(state, path, &AssetType::Folder).await {
                if exist.user_id() != user_id {
                    let msg = "you're attempting to create a folder that already beolngs to someone else.";
                    tracing::error!(msg);
                    return Err(AppError::InternalServerError(msg.to_string()));
                }
            }

            // build query values
            let pbq = bn.to_query(1);
            let uq = bn.to_query(2);
            let tq = bn.to_query(3);
            let pq = bn.to_query((index as u8) + 4);

            values.push(format!("({}, {}, {}, {})", pbq, uq, tq, pq));
        }

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
            is_public, user_id, asset_type;
            paths
        );

        tokio::fs::create_dir_all(parent).await?;
    }

    Ok(())
}

pub(super) async fn move_file(src: &PathBuf, dest: &Path) -> Result<(), AppError> {
    if let Err(err) = tokio::fs::File::create(dest).await {
        tracing::info!("unable to create destination file: {err}");
        return Err(AppError::IOError(err.to_string()));
    }

    tokio::fs::copy(&src, dest).await?;
    tokio::fs::remove_file(&src).await?;

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

pub(super) async fn create_or_update_asset<'a>(
    state: &AppState,
    opts: SaveAssetOpts<'a>,
    tmp: &Option<PathBuf>,
) -> Result<Asset, AppError> {
    let SaveAssetOpts {
        is_public,
        custom_path,
        user_id,
        path,
        asset_type,
    } = opts;

    match Asset::get_by_path(state, path, asset_type).await {
        Ok(exists) => {
            if &exists.user_id == user_id {
                update_asset(state, &exists.id, is_public, custom_path).await?;
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
    async fn test_rust_folders() -> Result<(), AppError> {
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

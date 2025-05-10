use std::path::{Path, PathBuf};

use crate::{
    errors::AppError,
    models::{permission::AssetPermission, user::User},
    routes::CreateAssetOptions,
    state::AppState,
    utils::sqlx_utils::{SqlxFilters, SqlxSetters, SqlxValues, ToQuery},
};

use super::{Asset, AssetSharing};

pub(super) struct SaveAssetOpts<'a> {
    pub path: &'a str,
    pub is_public: &'a Option<bool>,
    pub custom_path: &'a Option<String>,
    pub user_id: &'a i32,
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
    } = opts;

    let conn = state.db_pool().await;
    let bn = state.backend_name();

    let values = SqlxValues(4, 1).to_query(bn);
    let query = format!("INSERT INTO assets (asset_path, public, custom_path, user_id) {values}");
    sqlx::query(&query)
        .bind(path)
        .bind(is_public)
        .bind(custom_path)
        .bind(user_id)
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
    tmp: &Option<PathBuf>,
) -> Result<(), AppError> {
    let exist = Asset::get_by_path(state, custom_path).await;
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

/// Traverses asset path, create each parent dir and their respective records
/// where they don't exist.
pub(super) async fn create_asset_parents(
    state: &AppState,
    path: &Path,
    user_id: &i32,
    opt: &CreateAssetOptions,
) -> Result<(), AppError> {
    let CreateAssetOptions {
        public: is_public,
        custom_path,
        ..
    } = opt;

    let parent = path.parent();

    if let Some(parent) = parent {
        while let Some(path) = parent.ancestors().next() {
            if let Some(path) = path.to_str() {
                let exist = Asset::get_by_path(state, path).await;
                if let Ok(exist) = exist {
                    if exist.user_id() == user_id {
                        tracing::info!("parent '{path}' already exists. skipping...");
                        continue;
                    } else {
                        return Err(AppError::InternalServerError(
                            "you don't have access to asset parent: {path}".to_string(),
                        ));
                    }
                }

                let opts = SaveAssetOpts {
                    path,
                    is_public,
                    custom_path,
                    user_id,
                };

                save_asset(state, opts).await?;
            }
        }

        tokio::fs::create_dir_all(parent).await?;
    }

    Ok(())
}

pub(super) async fn move_file(src: &PathBuf, dest: &Path) -> Result<(), AppError> {
    if let Err(err) = tokio::fs::File::create(dest).await {
        tracing::info!("unble to create file: {err}");
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
) -> Result<(), AppError> {
    for opt in sharing {
        let fellow = User::get_by_pid(state, &opt.user_id).await;

        if let Err(err) = fellow {
            tracing::error!("error getting user to share asset with: {err}");
            continue;
        }

        let fellow = fellow?;
        let fellow_id = fellow.id();

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
    } = opts;

    match Asset::get_by_path(state, path).await {
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
            let asset = Asset::get_by_path(state, path).await?;

            Ok(asset)
        }
    }
}

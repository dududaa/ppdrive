use std::path::{Path, PathBuf};

use serde::Deserialize;
use tokio::fs::{create_dir_all, File};

use crate::{
    errors::AppError,
    models::user::User,
    state::AppState,
    utils::sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
};

use super::permission::{Permission, PermissionGroup};

#[derive(Default, Deserialize)]
pub enum AssetType {
    #[default]
    File,
    Folder,
}

#[derive(sqlx::FromRow)]
pub struct Asset {
    pub asset_path: String,
    pub user_id: i32,
    pub public: bool,
}

#[derive(Deserialize)]
pub struct AssetSharing {
    pub user_id: String,
    pub permission_group: PermissionGroup,
    pub permissions: Option<Vec<Permission>>,
}

#[derive(Default, Deserialize)]
pub struct CreateAssetOptions {
    /// Destination path where asset should be created
    pub path: String,

    /// The type of asset - whether it's a file or folder
    pub asset_type: AssetType,

    /// Asset's visibility. Public assets can be read/accessed by everyone. Private assets can be
    /// viewed ONLY by permission.
    pub public: Option<bool>,

    /// If `asset_type` is [AssetType::Folder], we determine whether we should force-create it's parents folder if they
    /// don't exist. Asset creation will result in error if `create_parents` is `false` and folder parents don't exist.
    pub create_parents: Option<bool>,

    /// Users to share this asset with. This can only be set if `public` option is false
    pub sharing: Vec<AssetSharing>,
}

impl Asset {
    pub async fn get_by_path(state: &AppState, path: &str) -> Result<Self, AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("asset_path").to_query(bn);
        let query = format!("SELECT * FROM assets WHERE {filters}");

        let asset = sqlx::query_as::<_, Asset>(&query)
            .bind(path)
            .fetch_one(&conn)
            .await?;

        Ok(asset)
    }

    pub async fn create_or_update(
        state: &AppState,
        user_id: &i32,
        opts: CreateAssetOptions,
        temp_file: Option<PathBuf>,
    ) -> Result<String, AppError> {
        let CreateAssetOptions {
            path,
            public: is_public,
            asset_type,
            create_parents,
            sharing,
        } = opts;

        let conn = state.db_pool().await;
        let user = User::get(state, user_id).await?;

        let folder = user.root_folder().as_deref();
        let path = folder.map_or(path.clone(), |rf| format!("{rf}/{path}"));

        let ap = Path::new(&path);

        // create the asset
        match asset_type {
            AssetType::File => {
                if let Some(parent) = ap.parent() {
                    if !parent.exists() {
                        create_dir_all(parent).await?;
                    }
                }

                if let Err(err) = File::create(ap).await {
                    tracing::info!("unble to create file: {err}");
                    return Err(AppError::IOError(err.to_string()));
                }

                if let Some(tmp) = temp_file {
                    tokio::fs::copy(&tmp, ap).await?;
                    tokio::fs::remove_file(&tmp).await?;
                }
            }
            AssetType::Folder => {
                if create_parents.unwrap_or_default() {
                    tokio::fs::create_dir_all(ap).await?
                } else {
                    tokio::fs::create_dir(ap).await?;
                }
            }
        }

        let bn = state.backend_name();
        let is_public = is_public.unwrap_or_default();

        // try to create asset record if it doesn't exist. If exists, update
        match Self::get_by_path(state, &path).await {
            Ok(exists) => {
                if &exists.user_id == user.id() {
                    let sf = SqlxFilters::new("public").to_query(bn);
                    let ff = SqlxFilters::new("user_id").to_query(bn);
                    let query = format!("UPDATE assets SET {sf} WHERE {ff}");

                    sqlx::query(&query)
                        .bind(is_public)
                        .bind(exists.user_id)
                        .execute(&conn)
                        .await?;
                } else {
                    tokio::fs::remove_file(&path).await?;
                    return Err(AppError::AuthorizationError(
                        "user has no permission to update asset".to_string(),
                    ));
                }
            }
            Err(_) => {
                let values = SqlxValues(3).to_query(bn);
                let query = format!("INSERT INTO assets (asset_path, public, user_id) {values}");
                sqlx::query(&query)
                    .bind(&path)
                    .bind(is_public)
                    .bind(user.id())
                    .execute(&conn)
                    .await?;
            }
        }

        Ok(path)
    }

    pub async fn delete_for_user(
        state: &AppState,
        user_id: &i32,
        remove_files: bool,
    ) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("user_id").to_query(bn);

        if remove_files {
            let query = format!("SELECT * FROM assets WHERE {filters}");
            let assets = sqlx::query_as::<_, Asset>(&query)
                .bind(user_id)
                .fetch_all(&conn)
                .await?;

            for asset in assets {
                asset.delete_object().await?;
            }
        }

        let query = format!("DELETE FROM assets WHERE {filters}");
        sqlx::query(&query).bind(user_id).execute(&conn).await?;

        Ok(())
    }

    async fn delete_object(&self) -> Result<(), AppError> {
        let path = Path::new(&self.asset_path);
        if path.is_file() {
            tokio::fs::remove_file(path).await?;
        } else if path.is_dir() {
            tokio::fs::remove_dir(path).await?;
        }

        Ok(())
    }
}

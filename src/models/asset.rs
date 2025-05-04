use std::path::{Path, PathBuf};

use serde::Deserialize;
use sqlx::AnyPool;
use tokio::fs::{create_dir_all, File};

use super::AssetType;
use crate::{errors::AppError, models::user::User, state::AppState};

#[derive(sqlx::FromRow)]
pub struct Asset {
    pub id: i32,
    pub asset_path: String,
    pub user_id: i32,
    pub public: bool,
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
}

impl Asset {
    pub async fn get_by_path(conn: &AnyPool, path: &str) -> Result<Self, AppError> {
        let asset = sqlx::query_as::<_, Asset>("SELECT * FROM assets WHERE asset_path = ?")
            .bind(path)
            .fetch_one(conn)
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
        } = opts;

        let conn = state.db_pool().await;
        let user = User::get(state, user_id).await?;
        let path = user
            .root_folder
            .map_or(path.clone(), |rf| format!("{rf}/{path}"));

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

        // try to create asset record if it doesn't exist. If exists, update
        match Self::get_by_path(&conn, &path).await {
            Ok(exists) => {
                if exists.user_id == user.id {
                    sqlx::query("UPDATE assets SET public = ? WHERE id = ?")
                        .bind(is_public.unwrap_or_default())
                        .bind(exists.id)
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
                sqlx::query(
                    r#"
                        INSERT INTO assets (asset_path, public, user_id)
                        VALUES(?, ?, ?)
                    "#,
                )
                .bind(&path)
                .bind(is_public.unwrap_or_default())
                .bind(user.id)
                .execute(&conn)
                .await?;
            }
        }

        Ok(path)
    }
}

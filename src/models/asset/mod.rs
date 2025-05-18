use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use ext::{
    create_asset_parents, create_or_update_asset, move_file, share_asset, validate_custom_path,
    SaveAssetOpts,
};
use serde::Deserialize;

use crate::{
    errors::AppError, models::user::User, routes::CreateAssetOptions, state::AppState,
    utils::sqlx::sqlx_utils::SqlxFilters,
};

use super::permission::{AssetPermission, Permission};

mod ext;

#[derive(Default, Deserialize)]
pub enum AssetType {
    #[default]
    File,
    Folder,
}

impl Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use AssetType::*;

        let value = match self {
            File => "File",
            Folder => "Folder",
        };

        write!(f, "{value}")
    }
}

impl TryFrom<i16> for AssetType {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        use AssetType::*;

        if value == 0 {
            Ok(File)
        } else if value == 1 {
            Ok(Folder)
        } else {
            Err(AppError::ParsingError(format!(
                "invalid asset_type {value}"
            )))
        }
    }
}

impl From<&AssetType> for i16 {
    fn from(value: &AssetType) -> Self {
        use AssetType::*;

        match value {
            File => 0,
            Folder => 1,
        }
    }
}

#[derive(sqlx::FromRow)]
pub struct Asset {
    id: i32,
    asset_path: String,
    custom_path: Option<String>,
    user_id: i32,
    public: bool,

    #[sqlx(try_from = "i16")]
    asset_type: AssetType,
}

impl Asset {
    pub async fn get_by_path(
        state: &AppState,
        path: &str,
        asset_type: &AssetType,
    ) -> Result<Self, AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let asset_type: i16 = asset_type.into();

        let filters = SqlxFilters::new("asset_path OR custom_path", 1)
            .add("AND asset_type")
            .to_query(bn)?;

        let query = format!("SELECT * FROM assets WHERE {filters}");

        let asset = sqlx::query_as::<_, Asset>(&query)
            .bind(path)
            .bind(path)
            .bind(asset_type)
            .fetch_one(&conn)
            .await?;

        Ok(asset)
    }

    pub async fn create_or_update(
        state: &AppState,
        user_id: &i32,
        opts: CreateAssetOptions,
        tmp: &Option<PathBuf>,
    ) -> Result<String, AppError> {
        let CreateAssetOptions {
            asset_path,
            public,
            asset_type,
            custom_path,
            create_parents,
            sharing,
        } = &opts;

        // validate custom_path
        if let Some(custom_path) = &custom_path {
            validate_custom_path(state, custom_path, asset_path, asset_type, tmp).await?;
        }

        let user = User::get(state, user_id).await?;
        let partition = user.partition().as_deref();
        let dest = partition.map_or(asset_path.clone(), |rf| format!("{rf}/{asset_path}"));
        let path = Path::new(&dest);

        // create asset parents (when they don't exist)
        let create_parents = create_parents.unwrap_or(true);
        let mut parent = None;
        if create_parents {
            parent = create_asset_parents(state, path, user_id, public).await?;
        }

        // create the asset
        match asset_type {
            AssetType::File => move_file(tmp, path).await?,
            AssetType::Folder => tokio::fs::create_dir(path).await?,
        }

        let is_public = public.unwrap_or_default();

        // try to create asset record if it doesn't exist. If exists,
        // update.
        let opts = SaveAssetOpts {
            path: &dest,
            is_public: &Some(is_public),
            custom_path: custom_path,
            user_id: user.id(),
            asset_type,
            parent: &parent,
        };

        let asset = create_or_update_asset(state, opts, tmp).await?;

        // create asset sharing as specified in options
        if !is_public {
            if let Some(sharing) = sharing {
                if !sharing.is_empty() {
                    share_asset(state, sharing, &asset.id, user_id).await?;
                }
            }
        }

        let path = asset.custom_path.unwrap_or(asset.asset_path);
        Ok(path)
    }

    pub async fn delete(&self, state: &AppState) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        // delete asset permissions
        let filters = SqlxFilters::new("id", 1).to_query(bn)?;
        AssetPermission::delete_for_asset(state, &self.id).await?;

        // delete children records
        self.delete_children_records(state).await?;

        // delete file/folder objects
        if let Err(err) = self.delete_object().await {
            tracing::error!("unable to remove {} from fs: {err}", self.asset_path)
        }

        // delete asset record
        sqlx::query(&format!("DELETE FROM assets WHERE {filters}"))
            .bind(&self.id)
            .execute(&conn)
            .await?;

        Ok(())
    }

    async fn delete_children_records(&self, state: &AppState) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filter = SqlxFilters::new("id", 1).to_query(bn)?;
        let query = format!("DELETE from assets WHERE {filter}");

        sqlx::query(&query).bind(&self.id).execute(&conn).await?;

        Ok(())
    }

    /// Delete assets for a specific user. Designed to handle bulk delete
    /// compared to `delete` method which handle delete for a specific
    /// asset.
    pub async fn delete_for_user(
        state: &AppState,
        user_id: &i32,
        remove_files: bool,
    ) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("user_id", 1).to_query(bn)?;

        if remove_files {
            let query = format!("SELECT * FROM assets WHERE {filters}");
            let assets = sqlx::query_as::<_, Asset>(&query)
                .bind(user_id)
                .fetch_all(&conn)
                .await?;

            for asset in assets {
                if let Err(err) = asset.delete_children_records(state).await {
                    tracing::error!("unable to delete children records for asset: {err}",)
                }

                if let Err(err) = asset.delete_object().await {
                    tracing::error!("unable to delete asset from {} fs: {err}", asset.asset_path)
                }
            }
        }

        let query = format!("DELETE FROM assets WHERE {filters}");
        sqlx::query(&query).bind(user_id).execute(&conn).await?;

        // delete all asset permissions for user
        AssetPermission::delete_for_user(state, user_id).await?;

        Ok(())
    }

    async fn delete_object(&self) -> Result<(), AppError> {
        let path = Path::new(&self.asset_path);
        if path.is_file() {
            tokio::fs::remove_file(path).await?;
        } else if path.is_dir() {
            tokio::fs::remove_dir_all(path).await?;
        }

        Ok(())
    }

    pub fn id(&self) -> &i32 {
        &self.id
    }

    pub fn public(&self) -> &bool {
        &self.public
    }

    pub fn path(&self) -> &str {
        &self.asset_path
    }

    pub fn custom_path(&self) -> &Option<String> {
        &self.custom_path
    }

    pub fn user_id(&self) -> &i32 {
        &self.user_id
    }

    pub fn url_path(&self) -> String {
        let default_path = format!("{}/{}", self.asset_type, self.asset_path);
        let up = self.custom_path.as_ref().unwrap_or(&default_path);
        up.to_string()
    }
}

#[derive(Deserialize)]
pub struct AssetSharing {
    pub user_id: String,
    pub permissions: Vec<Permission>,
}

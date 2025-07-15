use modeller::prelude::*;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use ext::{
    SaveAssetOpts, create_asset_parents, create_or_update_asset, share_asset, validate_custom_path,
};
use rbatis::{PageRequest, RBatis, crud, impl_select, impl_select_page};
use rbs::value;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{CoreResult, errors::CoreError, fs::move_file, options::CreateAssetOptions};

use super::{check_model, permission::AssetPermissions, user::Users};

mod ext;

#[derive(Default, Deserialize, Serialize)]
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

impl From<&AssetType> for u8 {
    fn from(value: &AssetType) -> Self {
        use AssetType::*;

        match value {
            File => 0,
            Folder => 1,
        }
    }
}

impl TryFrom<u8> for AssetType {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use AssetType::*;

        if value == 0 {
            Ok(File)
        } else if value == 1 {
            Ok(Folder)
        } else {
            Err(CoreError::ParseError("unrecognized asset_type".to_string()))
        }
    }
}

#[derive(Serialize, Deserialize, Modeller)]
pub struct Assets {
    #[modeller(serial)]
    id: Option<u64>,

    #[modeller(unique)]
    asset_path: String,
    custom_path: Option<String>,

    #[modeller(foreign_key(rf = "users(id)", on_delete = "cascade"))]
    user_id: u64,

    #[serde(deserialize_with = "de_sqlite_bool")]
    public: bool,
    asset_type: u8,
}

crud!(Assets {});

impl_select!(Assets{ select_by_path(path: &str, asset_type: u8) -> Option => "`WHERE (asset_path = #{path} OR custom_path = #{path}) AND asset_type = #{asset_type} LIMIT 1`" });
impl_select_page!(Assets { select_by_user(user_id: &u64) => "`WHERE user_id = #{user_id}`" });

impl Assets {
    pub async fn get_by_path(rb: &RBatis, path: &str, asset_type: &AssetType) -> CoreResult<Self> {
        let asset_type: u8 = asset_type.into();
        let asset = Assets::select_by_path(rb, path, asset_type).await?;

        check_model(asset, "asset not found")
    }

    pub async fn create_or_update(
        rb: &RBatis,
        user_id: &u64,
        opts: CreateAssetOptions,
        tmp: &Option<PathBuf>,
    ) -> Result<String, CoreError> {
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
            validate_custom_path(rb, custom_path, asset_path, asset_type, tmp).await?;
        }

        let user = Users::get(rb, user_id).await?;
        let partition = user.partition().as_deref();
        let dest = partition.map_or(asset_path.clone(), |rf| format!("{rf}/{asset_path}"));
        let path = Path::new(&dest);

        // create asset parents (when they don't exist)
        let create_parents = create_parents.unwrap_or(true);
        if create_parents {
            create_asset_parents(rb, path, user_id, public).await?;
        }

        // create the asset
        match asset_type {
            AssetType::File => move_file(tmp, path).await?,
            AssetType::Folder => tokio::fs::create_dir(path).await?,
        }

        let is_public = public.unwrap_or_default();

        // try to create asset record if it doesn't exist. If exists, update.
        let opts = SaveAssetOpts {
            path: &dest,
            is_public: &Some(is_public),
            custom_path: custom_path,
            user_id: &user.id(),
            asset_type,
        };

        let asset = create_or_update_asset(rb, opts, tmp).await?;

        // create asset sharing as specified in options
        if !is_public {
            if let Some(sharing) = sharing {
                if !sharing.is_empty() {
                    share_asset(rb, sharing, &asset.id(), user_id).await?;
                }
            }
        }

        let path = asset.custom_path.unwrap_or(asset.asset_path);
        Ok(path)
    }

    pub async fn delete(&self, rb: &RBatis) -> Result<(), CoreError> {
        // delete asset permissions
        AssetPermissions::delete_for_asset(rb, &self.id()).await?;

        // delete children records
        self.delete_children_records(rb).await?;

        // delete file/folder objects
        if let Err(err) = self.delete_object().await {
            tracing::error!("unable to remove {} from fs: {err}", self.asset_path)
        }

        // delete asset record
        Assets::delete_by_map(
            rb,
            value! {
                "id": &self.id
            },
        )
        .await?;

        Ok(())
    }

    async fn delete_children_records(&self, rb: &RBatis) -> Result<(), CoreError> {
        let t = &self.asset_type;
        let asset_type = AssetType::try_from(*t)?;

        if let AssetType::Folder = asset_type {
            let mut entries = tokio::fs::read_dir(&self.asset_path).await?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let child_type = if path.is_file() {
                    AssetType::File
                } else {
                    AssetType::Folder
                };

                if let Some(path) = path.to_str() {
                    if let Ok(child) = Assets::get_by_path(rb, path, &child_type).await {
                        Box::pin(child.delete(rb)).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Delete assets for a specific user. Designed to handle bulk delete
    /// compared to `delete` method which handle delete for a specific
    /// asset.
    pub async fn delete_for_user(
        rb: &RBatis,
        user_id: &u64,
        remove_files: bool,
    ) -> Result<(), CoreError> {
        // delete chidren files and associated
        if remove_files {
            let results = Assets::select_by_user(rb, &PageRequest::default(), user_id).await?;

            if results.page_size > 0 {
                for asset in results.records {
                    if let Err(err) = asset.delete_children_records(rb).await {
                        tracing::error!("unable to delete children records for asset: {err}",)
                    }

                    if let Err(err) = asset.delete_object().await {
                        tracing::error!(
                            "unable to delete asset from {} fs: {err}",
                            asset.asset_path
                        )
                    }
                }
            }
        }

        Assets::delete_by_map(
            rb,
            value! {
                "user_id": user_id
            },
        )
        .await?;

        // delete all asset permissions for user
        AssetPermissions::delete_for_user(rb, user_id).await?;

        Ok(())
    }

    async fn delete_object(&self) -> Result<(), CoreError> {
        let path = Path::new(&self.asset_path);
        if path.is_file() {
            tokio::fs::remove_file(path).await?;
        } else if path.is_dir() {
            tokio::fs::remove_dir_all(path).await?;
        }

        Ok(())
    }

    pub fn id(&self) -> u64 {
        *&self.id.unwrap_or_default()
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

    pub fn user_id(&self) -> &u64 {
        &self.user_id
    }

    pub fn url_path(&self) -> String {
        let t = &self.asset_type;
        let asset_type = AssetType::try_from(*t).ok().unwrap_or_default();

        let default_path = format!("{}/{}", asset_type, self.asset_path);
        let up = self.custom_path.as_ref().unwrap_or(&default_path);
        up.to_string()
    }
}

/// SQLite does not support boolean value directly. So we
/// deserialize `i64` to boolean;
fn de_sqlite_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = i64::deserialize(deserializer)?;
    Ok(v != 0)
}

use crate::{
    DBResult,
    errors::Error as AppError,
    models::{check_model, de_sqlite_bool},
};
use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select, impl_select_page};
use rbs::value;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

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
    type Error = AppError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use AssetType::*;

        if value == 0 {
            Ok(File)
        } else if value == 1 {
            Ok(Folder)
        } else {
            Err(AppError::ParseError("unrecognized asset_type".to_string()))
        }
    }
}

#[derive(Serialize, Deserialize, Modeller)]
pub struct Assets {
    id: Option<u64>,

    #[modeller(unique, length = 3000)]
    asset_path: String,

    #[modeller(length = 3000)]
    custom_path: Option<String>,

    #[modeller(foreign_key(rf = "users(id)", on_delete = "cascade"))]
    user_id: u64,

    #[modeller(foreign_key(rf = "buckets(id)", on_delete = "cascade"))]
    bucket_id: u64,

    #[serde(deserialize_with = "de_sqlite_bool")]
    public: bool,
    asset_type: u8,
}

crud!(Assets {});

impl_select!(Assets{ select_by_path(path: &str, asset_type: u8) -> Option => "`WHERE (asset_path = #{path} OR custom_path = #{path}) AND asset_type = #{asset_type} LIMIT 1`" });
impl_select_page!(Assets { select_by_user(user_id: &u64) => "`WHERE user_id = #{user_id}`" });

impl Assets {
    pub async fn get_by_path(rb: &RBatis, path: &str, asset_type: &AssetType) -> DBResult<Self> {
        let asset_type: u8 = asset_type.into();
        let asset = Assets::select_by_path(rb, path, asset_type).await?;

        check_model(asset, "asset not found")
    }

    pub async fn delete_for_user(db: &RBatis, user_id: &u64) -> DBResult<()> {
        Assets::delete_by_map(db, value! { "user_id": user_id }).await?;
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

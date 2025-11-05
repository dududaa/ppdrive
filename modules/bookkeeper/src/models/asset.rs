use crate::{
    DBResult,
    errors::Error as AppError,
    models::{
        check_model, de_sqlite_bool,
        permission::{AssetPermissions, Permission},
        user::Users,
    },
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
    slug: Option<String>,

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
    pub async fn get_by_path(db: &RBatis, path: &str, asset_type: &AssetType) -> DBResult<Self> {
        let asset_type: u8 = asset_type.into();
        let asset = Assets::select_by_path(db, path, asset_type).await?;

        check_model(asset, "asset not found")
    }

    pub async fn insert_group(db: &RBatis, values: Vec<NewAsset>) -> DBResult<()> {
        let mut tables = Vec::with_capacity(values.len());

        for v in values {
            tables.push(v.into());
        }

        Assets::insert_batch(db, &tables, tables.len() as u64).await?;
        Ok(())
    }

    pub async fn update(&mut self, db: &RBatis, values: UpdateAssetValues) -> DBResult<()> {
        let UpdateAssetValues {
            public,
            custom_path,
            asset_path,
        } = values;

        self.public = public;
        self.slug = custom_path;
        self.asset_path = asset_path;

        Assets::update_by_map(db, self, value! { "id": &self.id() }).await?;
        Ok(())
    }

    pub async fn create(db: &RBatis, value: NewAsset) -> DBResult<()> {
        Assets::insert(db, &value.into()).await?;
        Ok(())
    }

    pub async fn delete_for_user(db: &RBatis, user_id: &u64) -> DBResult<()> {
        let assets = Assets::select_by_map(db, value! { "user_id": user_id }).await?;
        for asset in assets {
            asset.delete(db).await?;
        }

        Ok(())
    }

    pub async fn delete(&self, db: &RBatis) -> DBResult<()> {
        // delete asset permissions
        AssetPermissions::delete_for_asset(db, &self.id()).await?;

        // delete asset record
        Assets::delete_by_map(
            db,
            value! {
                "id": &self.id
            },
        )
        .await?;

        Ok(())
    }

    /// update asset sharing and ownership record
    pub async fn share(&self, db: &RBatis, sharing: &Vec<AssetSharing>) -> DBResult<()> {
        for opt in sharing {
            let get_fellow = Users::get_by_pid(db, &opt.user_id).await;
            if let Err(err) = get_fellow {
                tracing::error!("error getting user to share asset with: {err}");
                continue;
            }

            let fellow = get_fellow?;
            let fellow_id = &fellow.id();
            if &self.user_id == fellow_id {
                tracing::error!("you cannot share asset {} with it's owner", self.id());
                continue;
            }

            if opt.permissions.is_empty() {
                tracing::error!("permissions list must be specifed for a sharing option");
                continue;
            }

            for permission in &opt.permissions {
                AssetPermissions::create(db, &self.user_id, fellow_id, permission.clone()).await?;
            }
        }

        Ok(())
    }

    /// checks if a user has read access to the asset
    pub async fn can_read(&self, db: &RBatis, user_id: &u64) -> DBResult<()> {
        AssetPermissions::exists(db, user_id, &self.id(), Permission::Read).await?;
        Ok(())
    }

    pub fn id(&self) -> u64 {
        self.id.unwrap_or_default()
    }

    pub fn public(&self) -> &bool {
        &self.public
    }

    pub fn path(&self) -> &str {
        &self.asset_path
    }

    pub fn custom_path(&self) -> &Option<String> {
        &self.slug
    }

    pub fn user_id(&self) -> &u64 {
        &self.user_id
    }

    pub fn url_path(&self) -> String {
        let t = &self.asset_type;
        let asset_type = AssetType::try_from(*t).ok().unwrap_or_default();

        let default_path = format!("{}/{}", asset_type, self.asset_path);
        let up = self.slug.as_ref().unwrap_or(&default_path);
        up.to_string()
    }
}

pub struct NewAsset {
    pub asset_path: String,
    pub custom_path: Option<String>,
    pub user_id: u64,
    pub bucket_id: u64,
    pub public: bool,
    pub asset_type: u8,
}

impl From<NewAsset> for Assets {
    fn from(value: NewAsset) -> Self {
        let NewAsset {
            asset_path,
            custom_path,
            user_id,
            bucket_id,
            public,
            asset_type,
        } = value;

        Assets {
            id: None,
            asset_path,
            slug: custom_path,
            user_id,
            bucket_id,
            public,
            asset_type,
        }
    }
}

pub struct UpdateAssetValues {
    pub public: bool,
    pub custom_path: Option<String>,
    pub asset_path: String,
}

#[derive(Deserialize, Serialize)]
pub struct AssetSharing {
    pub user_id: String,
    pub permissions: Vec<Permission>,
}

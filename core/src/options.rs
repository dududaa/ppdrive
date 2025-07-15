use serde::{Deserialize, Serialize};

use crate::models::{asset::AssetType, permission::Permission, user::UserRole};

#[derive(Deserialize)]
pub struct AssetSharing {
    pub user_id: String,
    pub permissions: Vec<Permission>,
}

#[derive(Default, Deserialize)]
pub struct CreateAssetOptions {
    /// Destination path where asset should be created
    pub asset_path: String,

    /// The type of asset - whether it's a file or folder
    pub asset_type: AssetType,

    /// Asset's visibility. Public assets can be read/accessed by everyone. Private assets can be
    /// viewed ONLY by permission.
    pub public: Option<bool>,

    /// Set a custom path for your asset instead of the one auto-generated from
    /// from `path`. This useful if you'd like to conceal your original asset path.
    /// Custom path must be available in that no other asset is already using it in the entire app.
    ///
    /// Your original asset path makes url look like this `https://mydrive.com/images/somewhere/my-image.png/`.
    /// Using custom path, you can conceal the original path: `https://mydrive.com/some/hidden-path`
    pub custom_path: Option<String>,

    /// If `asset_type` is [AssetType::Folder], we determine whether we should force-create it's parents folder if they
    /// don't exist. Asset creation will result in error if `create_parents` is `false` and folder parents don't exist.
    pub create_parents: Option<bool>,

    /// Users to share this asset with. This can only be set if `public` option is false
    pub sharing: Option<Vec<AssetSharing>>,
}

#[derive(Deserialize)]
pub struct CreateUserOptions {
    pub partition: Option<String>,
    pub partition_size: Option<u64>,
    pub role: UserRole,
}

#[derive(Deserialize, Serialize, Default)]
pub struct CreateBucketOptions {
    pub max_size: Option<u64>,
    pub root_folder: Option<String>,
    pub accepts: Option<String>,
}

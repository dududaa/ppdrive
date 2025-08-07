use serde::{Deserialize, Serialize};

use crate::models::{asset::AssetType, permission::Permission};

#[derive(Deserialize, Serialize)]
pub struct CreateUserClient {
    pub max_bucket: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct LoginUserClient {
    pub id: String,
    pub access_exp: Option<i64>,
    pub refresh_exp: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginToken {
    pub access: (String, i64),
    pub refresh: (String, i64),
}

#[derive(Deserialize, Serialize)]
pub struct AssetSharing {
    pub user_id: String,
    pub permissions: Vec<Permission>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct CreateAssetOptions {
    /// Destination path where asset should be created
    pub asset_path: String,

    /// The type of asset - whether it's a file or folder
    pub asset_type: AssetType,

    /// The UID of bucket in which to save the asset
    pub bucket: String,

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

#[derive(Deserialize, Serialize, Default)]
pub struct CreateBucketOptions {
    pub partition: Option<String>,

    /// can be set if there's partition
    pub partition_size: Option<u64>,

    /// The mime type acceptable by a bucket.
    /// - "*" is the default and means all mime types are accepted.
    /// - "custom" means a selection of mimetypes manually specified
    /// by a user. Acceptable format should start with "custom" keyword
    /// followed by a colon ":" and comma seprated mimetypes. Example, "custom:application/zip,audio/3gpp"
    /// - You can specify a group of mimes using the `filetype` they
    /// belong to (e.g, "audio", "video", "application"...etc).
    /// - You can also specify a *list* of comma seprated groups e.g, "audio,video,application".
    pub accepts: String,

    pub label: String,
    pub public: Option<bool>,
}

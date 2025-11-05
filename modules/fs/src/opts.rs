use ppd_bk::models::asset::{AssetSharing, AssetType};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
pub struct CreateAssetOptions {
    /// Destination path where asset should be created
    pub asset_path: String,

    /// The type of asset - whether it's a file or folder.
    pub asset_type: AssetType,

    /// The public ID of the bucket in which to save the asset.
    pub bucket: String,

    /// Asset's visibility. If true, asset can be read/accessed by everyone. Else, asset can be
    /// viewed ONLY by permission.
    pub public: Option<bool>,

    /// Set a custom URL slug for your asset instead of the one auto-generated  from `asset_path`. 
    /// This is useful if you'd like to conceal your original asset path. Custom path must be 
    /// unique in that no other asset in entire PPDRIVE instance is already using it.
    ///
    /// Your original asset slug makes url look like this `https://mydrive.com/images/somewhere/my-image.png/`.
    /// Using custom slug, you can conceal the original path: `https://mydrive.com/some/hidden-path`.
    pub slug: Option<String>,

    /// Create asset's parent folders if they don't already exist. The endpoint will return an 
    /// error if `create_parents` is `false` and folder parents don't exist.
    pub create_parents: Option<bool>,

    /// Determine whether to share asset with other users. This is mostly useful for private assets.
    pub sharing: Option<Vec<AssetSharing>>,

    /// overwrite existing asset.
    pub overwrite: Option<bool>
}
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Serialize, Deserialize, Validate, Default)]
pub struct UploadUrlConfig {
    pub method: UploadUrlMethod,
    pub asset_type: AssetType,
    #[validate(range(min = 30))]
    pub expires: i32,
    pub path: String,
    pub filesize: Option<u64>,
    /// Create asset parent folders if they don't exist, else error will be returned.
    pub create_parents: Option<bool>,
    /// overwrite asset if it already exists.
    pub overwrite: Option<bool>,
    pub resumable: Option<bool>,
}

#[derive(Serialize, Deserialize, Default)]
pub enum UploadUrlMethod {
    #[default]
    Post,
    Put,
}

#[derive(Serialize, Deserialize, Default)]
pub enum AssetType {
    #[default]
    File,
    Folder,
}

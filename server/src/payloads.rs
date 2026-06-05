use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Serialize, Deserialize, Validate)]
pub struct UploadUrlConfig {
    pub method: UploadUrlMethod,
    pub asset_type: AssetType,
    #[validate(range(min = 30))]
    pub expires: i32,
    // Create asset parent folders if they don't exist, else error will be returned.
    pub create_parents: Option<bool>,
    // overwrite asset if it already exists
    pub overwrite: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub enum UploadUrlMethod { Post, Put }

#[derive(Serialize, Deserialize)]
pub enum AssetType { File, Folder }
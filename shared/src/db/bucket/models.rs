use sqlx::FromRow;
use crate::AssetOwnerName;

#[derive(Default, Debug)]
pub struct CreateBucketData {
    pub name: String,
    pub path: String,
    pub owner_type: AssetOwnerName,
    pub owner_id: i32,
    pub public: bool,
    pub size: Option<i64>,
    pub accepts: Option<Vec<String>>,
}

#[derive(FromRow)]
pub struct Bucket {
    pub name: String,
    pub path: String,
    pub public: bool,
    pub size: Option<i64>,
    pub accepts: Option<Vec<String>>,
}
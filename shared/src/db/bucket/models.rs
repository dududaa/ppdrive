//! Bucket database models.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::AssetOwnerName;
use validator::Validate;

/// Data required to create a new bucket.
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

/// Client-facing request body for `POST /buckets`.
#[cfg(feature = "server")]
#[derive(Serialize, Deserialize, Validate)]
pub struct CreateBucketRequest {
    /// Human-readable bucket name (1–255 chars).
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    /// Storage path relative to the root directory (must not be empty).
    #[validate(length(min = 1))]
    pub path: String,
    /// Whether the bucket is publicly readable (default: false).
    #[serde(default)]
    pub public: bool,
    /// Maximum bucket size in megabytes (None = unlimited).
    pub size: Option<i64>,
    /// Accepted MIME types (e.g. `["image/*", "application/pdf"]`).
    pub accepts: Option<Vec<String>>,
}

/// Bucket row returned from the database.
#[derive(FromRow)]
pub struct Bucket {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub public: bool,
    pub size: Option<i64>,
    pub accepts: Option<Vec<String>>,
}
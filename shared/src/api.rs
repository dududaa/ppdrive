use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct CreateClientUser {
    /// Total size of buckets the user can create (MB). This is the total accumulated size, which means
    /// the user can create as many buckets as possible as long as the total size of all the buckets
    /// combined doesn't exceed this size. 
    /// 
    /// When the option is not specified, user can create unlimited number of buckets.
    pub max_bucket: Option<f64>,
}

#[derive(Deserialize, Serialize)]
pub struct LoginUserClient {
    pub id: String,
    pub access_exp: Option<i64>,
    pub refresh_exp: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginTokens {
    pub access: Option<(String, i64)>,
    pub refresh: Option<(String, i64)>,
}

#[derive(Deserialize, Serialize)]
pub struct UserCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, Serialize, Default)]
pub struct CreateBucketOptions {
    pub partition: Option<String>,

    /// can be set if there's partition
    pub partition_size: Option<f64>,

    /// The mime type acceptable by a bucket.
    /// - "*" is the default and means all mime types are accepted.
    /// - "custom" means a selection of mimetypes manually specified by a user. Acceptable format should start with "custom" keyword followed by a colon ":" and comma seprated mimetypes. Example, "custom:application/zip,audio/3gpp"
    /// - You can specify a group of mimes using the `filetype` they belong to (e.g, "audio", "video", "application"...etc).
    /// - You can also specify a *list* of comma seprated groups e.g, "audio,video,application".
    pub accepts: Option<String>,

    pub label: String,
    pub public: Option<bool>,
}
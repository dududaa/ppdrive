use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

use crate::{impl_validator, opts::OptionValidator};

#[derive(Deserialize, Serialize, Validate)]
pub struct CreateClientUser {
    /// Total size of buckets the user can create (MB). This is the total accumulated size, which means
    /// the user can create as many buckets as possible as long as the total size of all the buckets
    /// combined doesn't exceed this size.
    ///
    /// When the option is not specified, user can create unlimited number of buckets.
    pub max_bucket: Option<f64>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct LoginUserClient {
    #[validate(length(
        min = 8,
        max = 120,
        message = "'id' length must be between 8 to 12 characters."
    ))]
    pub id: String,

    #[validate(range(min = 900))]
    pub access_exp: Option<i64>,

    #[validate(range(min = 900))]
    pub refresh_exp: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginTokens {
    pub access: Option<(String, i64)>,
    pub refresh: Option<(String, i64)>,
}

#[derive(Deserialize, Serialize, Validate)]
pub struct UserCredentials {
    #[validate(length(
        min = 8,
        max = 120,
        message = "'username' length must be between 8 to 12 characters."
    ))]
    pub username: String,

    #[validate(custom(function = "validate_password_complexity"))]
    pub password: String,
}

#[derive(Deserialize, Serialize, Default, Validate)]
pub struct CreateBucketOptions {
    #[validate(length(min=8))]
    pub label: String,

    #[validate(length(min=2))]
    pub root_path: Option<String>,

    /// can be set if there's `partition_path`
    #[validate(range(min = 0.1))]
    pub size: Option<f64>,

    /// The mime type acceptable by a bucket.
    /// - "*" is the default and means all mime types are accepted.
    /// - "custom" means a selection of mimetypes manually specified by a user. Acceptable format should start with "custom" keyword followed by a colon ":" and comma seprated mimetypes. Example, "custom:application/zip,audio/3gpp"
    /// - You can specify a group of mimes using the `filetype` they belong to (e.g, "audio", "video", "application"...etc).
    /// - You can also specify a *list* of comma seprated groups e.g, "audio,video,application".
    #[validate(length(min=1))]
    pub accepts: Option<String>,

    pub public: Option<bool>,
}

static HAS_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d").unwrap());
static HAS_SPECIAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[!@#$%^&*(),.?":{}|<>]"#).unwrap());

fn validate_password_complexity(password: &str) -> Result<(), ValidationError> {
    if !HAS_NUMBER.is_match(password) {
        return Err(ValidationError::new("password_no_number"));
    }
    if !HAS_SPECIAL.is_match(password) {
        return Err(ValidationError::new("password_no_special"));
    }
    Ok(())
}

impl_validator!(LoginUserClient);
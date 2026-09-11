use clap::{Args, Subcommand};
use shared::db::bucket::models::CreateBucketData;
use shared::db::asset::models::PermissionLevel;

#[derive(Subcommand, Debug)]
pub enum ClientCommand {
    /// create a new client and receive the client token.
    Create {
        /// Arbitrary name to remember the client. Use a name that describes the client application(s), e.g MyGoodness App
        #[arg(long("name"))]
        client_name: String,
    },

    /// refresh token for a given client.
    Refresh {
        #[arg(long("id"))]
        client_id: String,
    },

    List,
}

#[derive(Default, Args, Debug, Clone)]
pub struct CreateBucketArgs {
    /// Bucket name
    #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub name: String,

    /// Bucket's root path (must not contain '..')
    #[arg(long, value_parser = validate_bucket_path)]
    pub path: String,

    /// Type of entity that owns the bucket.
    #[arg(long, value_enum)]
    pub owner_type: shared::AssetOwnerName,

    /// ID of entity that owns the bucket.
    #[arg(long, value_parser = clap::builder::NonEmptyStringValueParser::new())]
    pub owner_id: String,

    /// Determines whether bucket is public or private.
    #[arg(long)]
    pub public: bool,

    /// Size (bytes) of the bucket.
    #[arg(long)]
    pub size: Option<i64>,

    /// Mime types acceptable by the bucket.
    #[arg(long)]
    pub accepts: Option<Vec<String>>,
}

/// Validate that a bucket path is non-empty and contains no `..` components.
fn validate_bucket_path(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Err("path must not be empty".into());
    }
    if s.contains("..") {
        return Err("path must not contain '..'".into());
    }
    Ok(s.to_string())
}

impl CreateBucketArgs {
    /// Convert CLI args into the database insert model, resolving the owner PID.
    pub fn into_data(self, resolved_owner_id: i32) -> CreateBucketData {
        CreateBucketData {
            name: self.name,
            path: self.path,
            owner_type: self.owner_type,
            owner_id: resolved_owner_id,
            public: self.public,
            size: self.size,
            accepts: self.accepts,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum BucketCommand {
    Create(CreateBucketArgs),
}

#[derive(Subcommand, Debug)]
pub enum AssetCommand {
    /// Grant a permission on a file in a private bucket
    Grant {
        /// PID of the bucket
        #[arg(long)]
        bucket: String,
        /// Relative path of the file within the bucket
        #[arg(long)]
        path: String,
        /// PID of the client or email of the user to grant access to
        #[arg(long)]
        grantee: String,
        /// Type of grantee: "client" or "user" (default: client)
        #[arg(long, default_value = "client")]
        grantee_type: String,
        /// Permission level: read, write, or admin
        #[arg(long, value_enum)]
        permission: PermissionLevel,
    },
    /// Revoke a permission on a file in a private bucket
    Revoke {
        /// PID of the bucket
        #[arg(long)]
        bucket: String,
        /// Relative path of the file within the bucket
        #[arg(long)]
        path: String,
        /// PID of the client or email of the user to revoke access from
        #[arg(long)]
        grantee: String,
        /// Type of grantee: "client" or "user" (default: client)
        #[arg(long, default_value = "client")]
        grantee_type: String,
    },
    /// List permissions for files in a private bucket
    List {
        /// PID of the bucket
        #[arg(long)]
        bucket: String,
        /// Optional relative path to filter by a specific file
        #[arg(long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum UserCommand {
    /// Create a new user account
    Create {
        /// User email address
        #[arg(long)]
        email: String,
        /// User password
        #[arg(long)]
        password: String,
    },
}

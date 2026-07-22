use clap::{Args, Subcommand};
use shared::db::buckets::CreateBucketData;
use shared::db::Database;
use shared::{AssetOwnerName, db::{client, buckets}};

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
    #[arg(long)]
    name: String,

    /// Bucket's root path
    #[arg(long)]
    path: String,

    /// Type of entity that owns the bucket.
    #[arg(long, value_enum)]
    owner_type: AssetOwnerName,

    /// ID of entity that owns the bucket.
    #[arg(long)]
    owner_id: String,

    /// Determines whether bucket is public or private.
    #[arg(long)]
    public: bool,

    /// Size (bytes) of the bucket.
    #[arg(long)]
    size: Option<i64>,

    /// Mime types acceptable by the bucket.
    #[arg(long)]
    accepts: Option<Vec<String>>,
}

impl CreateBucketArgs {
    pub async fn insert(self, db: &Database) -> anyhow::Result<String> {
        let id = client::get_id(&self.owner_id, db).await?;
        let Self {
            name,
            path,
            owner_type,
            public,
            size,
            accepts,
            ..
        } = self;

        let data = CreateBucketData {
            name,
            path,
            owner_type,
            owner_id: id,
            public,
            size,
            accepts,
        };

        let id = buckets::create(&data, db).await?;
        Ok(id)
    }
}

#[derive(Subcommand, Debug)]
pub enum BucketCommand {
    Create(CreateBucketArgs),
}

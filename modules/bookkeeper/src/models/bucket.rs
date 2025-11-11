use crate::{
    DBResult, Error as AppError,
    models::{
        de_sqlite_bool,
        mime::{BucketMimes, Mimes},
    },
};
use modeller::prelude::*;
use ppd_shared::opts::api::CreateBucketOptions;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Modeller)]
pub struct Buckets {
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,

    owner_id: u64,
    owner_type: u8,

    #[modeller(length = 256)]
    label: String,

    partition: Option<String>,

    /// can be set if there's partition
    partition_size: Option<f64>,

    #[modeller(default = "*")]
    accepts: String,

    #[serde(deserialize_with = "de_sqlite_bool")]
    public: bool,
}

crud!(Buckets {});

impl_select!(Buckets { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Buckets {
    pub async fn get_by_pid(db: &RBatis, pid: &str) -> DBResult<Self> {
        let s = Self::get_by_key(db, "pid", pid)
            .await?
            .ok_or(AppError::NotFound("bucket not found".to_string()))?;

        Ok(s)
    }

    pub async fn create_by_client(
        db: &RBatis,
        opts: CreateBucketOptions,
        client_id: u64,
    ) -> DBResult<String> {
        let owner_info = OwnerInfo {
            id: client_id,
            ty: BucketOwnerType::Client,
        };

        let id = Self::create(db, opts, owner_info).await?;
        Ok(id)
    }

    async fn user_buckets(db: &RBatis, user_id: &u64) -> DBResult<Vec<Buckets>> {
        let owner_type = u8::from(BucketOwnerType::User);
        let buckets = Buckets::select_by_map(
            db,
            value! {
                "owner_id": user_id,
                "owner_type": owner_type
            },
        )
        .await?;

        Ok(buckets)
    }

    async fn client_buckets(db: &RBatis, client_id: &u64) -> DBResult<Vec<Buckets>> {
        let owner_type = u8::from(BucketOwnerType::Client);
        let buckets = Buckets::select_by_map(
            db,
            value! {
                "owner_id": client_id,
                "owner_type": owner_type
            },
        )
        .await?;

        Ok(buckets)
    }

    pub async fn user_total_bucket_size(db: &RBatis, user_id: &u64) -> DBResult<f64> {
        let buckets = Buckets::user_buckets(db, user_id).await?;
        let size = buckets
            .iter()
            .fold(0f64, |acc, b| acc + b.partition_size.unwrap_or_default());

        Ok(size)
    }

    pub async fn client_total_bucket_size(db: &RBatis, client_id: &u64) -> DBResult<f64> {
        let buckets = Buckets::client_buckets(db, client_id).await?;
        let size = buckets
            .iter()
            .fold(0f64, |acc, b| acc + b.partition_size.unwrap_or_default());

        Ok(size)
    }

    pub async fn create_by_user(
        db: &RBatis,
        opts: CreateBucketOptions,
        user_id: u64,
    ) -> DBResult<String> {
        let owner_info = OwnerInfo {
            id: user_id,
            ty: BucketOwnerType::User,
        };

        let id = Self::create(db, opts, owner_info).await?;
        Ok(id)
    }

    pub async fn delete(db: &RBatis, pid: &str) -> DBResult<()> {
        Self::delete_by_map(db, value! { "pid": pid }).await?;
        Ok(())
    }

    /// validate whether a given user can write to this bucket
    pub fn validate_write(&self, user_id: &u64) -> bool {
        if !self.public()
            && let BucketOwnerType::User = self.owner_type()
            && self.owner_id() != user_id
        {
            return false;
        }

        true
    }

    /// save bucket's acceptable mimetypes based on `accepts` parameter.
    pub async fn save_mimes(&self, db: &RBatis) -> DBResult<()> {
        let accepts = &self.accepts;

        if accepts == "*" {
            return Ok(());
        }

        let mut mime_ids = Vec::new();
        if accepts.starts_with("custom") {
            let mstr = accepts.split(":").collect::<Vec<&str>>();
            match mstr.get(1) {
                Some(mlist) => {
                    let mimes = mlist.split(",").collect::<Vec<&str>>();
                    for mime in mimes {
                        let get_mimes = Mimes::select_by_map(
                            db,
                            value! {
                                "mime": mime.trim()
                            },
                        )
                        .await?;

                        mime_ids = get_mimes.iter().map(|m| m.id()).collect();
                    }
                }
                None => {
                    return Err(AppError::PermissionError(
                        "You need to specify mime list for custom mimetypes.".to_string(),
                    ));
                }
            }
        } else {
            let filetypes = accepts.split(",").collect::<Vec<&str>>();
            for filtype in filetypes {
                let get_mimes = Mimes::select_by_map(
                    db,
                    value! {
                        "filetype": filtype.trim()
                    },
                )
                .await?;

                let mut ids = get_mimes.iter().map(|m| m.id()).collect();
                mime_ids.append(&mut ids);
            }
        }

        let bucket_id = &self.id.unwrap_or_default();
        if !mime_ids.is_empty() {
            for mime_id in mime_ids {
                let query = format!(
                    "INSERT INTO bucket_mimes (bucket_id, mime_id) VALUES ({bucket_id}, {mime_id})"
                );

                if let Err(err) = db.exec(&query, vec![]).await {
                    println!("{err}")
                }
            }
        }

        Ok(())
    }

    pub async fn validate_mime(&self, db: &RBatis, mime: &str) -> DBResult<()> {
        if self.accepts == "*" {
            return Ok(());
        }

        let get_mime = Mimes::get_by_key(db, "mime", mime).await?;
        let mime = get_mime.ok_or(AppError::PermissionError(format!(
            "unsupported file mime '{mime}'"
        )))?;

        let bucket_mimes = BucketMimes::select_by_bucket(db, &self.id()).await?;
        let exists = bucket_mimes.iter().find(|bm| *bm.mime_id() == mime.id());

        match exists {
            Some(_) => Ok(()),
            None => Err(AppError::PermissionError(format!(
                "mime '{}' not supported by selected bucket.",
                mime.mime()
            ))),
        }
    }

    async fn create(
        db: &RBatis,
        opts: CreateBucketOptions,
        owner_info: OwnerInfo,
    ) -> DBResult<String> {
        let OwnerInfo { id: owner_id, ty } = owner_info;
        let owner_type = u8::from(ty);

        let CreateBucketOptions {
            size: partition_size,
            root_path: partition,
            accepts,
            label,
            public,
        } = opts;

        if let Some(size) = partition_size
            && size < 0.0
        {
            return Err(AppError::PermissionError(
                "partition_size must be minimum of 1".to_string(),
            ));
        }

        if let Some(folder) = &partition {
            if folder.len() < 6 {
                return Err(AppError::PermissionError(
                    "\"partitition\" must be more than 6 characters.".to_string(),
                ));
            }

            let b = Buckets::get_by_key(db, "root_folder", folder).await?;
            if b.is_some() {
                return Err(AppError::PermissionError(format!(
                    "folder name '{folder}' is not available. Try a different folder name."
                )));
            }
        }

        if partition_size.is_some() && partition.is_none() {
            return Err(AppError::PermissionError(
                "You can not set \"partition_size\" without setting \"partition\".".to_string(),
            ));
        }

        let accepts = accepts.unwrap_or(String::from("*"));
        let pid = Uuid::new_v4().to_string();

        let data = Buckets {
            id: None,
            pid,
            owner_id,
            owner_type,
            label,
            partition_size,
            partition,
            accepts,
            public: public.unwrap_or_default(),
        };

        Buckets::insert(db, &data).await?;

        if let Ok(bucket) = Buckets::get_by_pid(db, &data.pid).await
            && let Err(err) = bucket.save_mimes(&db).await
        {
            tracing::error!("unable to save mimes: {err}");
        }

        Ok(data.pid)
    }

    pub fn id(&self) -> u64 {
        self.id.unwrap_or_default()
    }

    pub fn owner_id(&self) -> &u64 {
        &self.owner_id
    }

    pub fn owner_type(&self) -> BucketOwnerType {
        self.owner_type.into()
    }

    pub fn public(&self) -> bool {
        self.public
    }

    pub fn partition(&self) -> &Option<String> {
        &self.partition
    }

    pub fn partition_size(&self) -> &Option<f64> {
        &self.partition_size
    }

    pub fn accepts(&self) -> &str {
        &self.accepts
    }
}

pub enum BucketOwnerType {
    Client,
    User,
}

impl From<BucketOwnerType> for u8 {
    fn from(value: BucketOwnerType) -> Self {
        use BucketOwnerType::*;

        match value {
            Client => 0,
            User => 1,
        }
    }
}

impl From<u8> for BucketOwnerType {
    fn from(value: u8) -> Self {
        use BucketOwnerType::*;

        if value == 0 { Client } else { User }
    }
}

struct OwnerInfo {
    id: u64,
    ty: BucketOwnerType,
}

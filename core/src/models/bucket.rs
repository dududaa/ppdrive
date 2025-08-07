use std::path::Path;

use crate::models::{
    de_sqlite_bool,
    mime::{BucketMimes, Mimes},
};
use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreResult, errors::CoreError, options::CreateBucketOptions, tools::get_folder_size};

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
    partition_size: Option<u64>,

    #[modeller(default = "*")]
    accepts: String,

    #[serde(deserialize_with = "de_sqlite_bool")]
    public: bool,
}

crud!(Buckets {});

impl_select!(Buckets { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Buckets {
    pub async fn create_by_client(
        db: &RBatis,
        opts: CreateBucketOptions,
        client_id: u64,
    ) -> Result<String, CoreError> {
        let owner_info = OwnerInfo {
            id: client_id,
            ty: BucketOwnerType::Client,
        };

        let id = Self::create(db, opts, owner_info).await?;
        Ok(id)
    }

    pub async fn create_by_user(
        db: &RBatis,
        opts: CreateBucketOptions,
        user_id: u64,
    ) -> Result<String, CoreError> {
        let owner_info = OwnerInfo {
            id: user_id,
            ty: BucketOwnerType::User,
        };

        let id = Self::create(db, opts, owner_info).await?;
        Ok(id)
    }

    pub async fn get_by_pid(db: &RBatis, pid: &str) -> CoreResult<Self> {
        let s = Self::get_by_key(db, "pid", pid)
            .await?
            .ok_or(CoreError::NotFound("bucket not found".to_string()))?;

        Ok(s)
    }

    pub async fn delete(db: &RBatis, pid: &str) -> CoreResult<()> {
        Self::delete_by_map(db, value! { "pid": pid }).await?;
        Ok(())
    }

    pub async fn content_size(&self) -> CoreResult<u64> {
        let mut size = 0;
        if let Some(partition) = &self.partition {
            let dir = Path::new(partition);
            if !dir.exists() {
                tokio::fs::create_dir_all(dir).await?;
                return Ok(size);
            }

            get_folder_size(partition, &mut size).await?;
        }

        Ok(size)
    }

    pub fn id(&self) -> u64 {
        *&self.id.unwrap_or_default()
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

    pub fn partition_size(&self) -> &Option<u64> {
        &self.partition_size
    }

    async fn create(
        db: &RBatis,
        opts: CreateBucketOptions,
        owner_info: OwnerInfo,
    ) -> CoreResult<String> {
        let OwnerInfo { id: owner_id, ty } = owner_info;
        let owner_type = u8::from(ty);

        let CreateBucketOptions {
            partition_size,
            partition,
            accepts,
            label,
            public,
        } = opts;

        if accepts.is_empty() {
            return Err(CoreError::PermissionError(format!(
                "bucket's \"accept\" parameter cannot be empty. please check docs to see how to specify acceptable mimetypes."
            )));
        }

        if let Some(folder) = &partition {
            let b = Buckets::get_by_key(db, "root_folder", folder).await?;
            if b.is_some() {
                return Err(CoreError::PermissionError(format!(
                    "folder name '{folder}' is not available. Try a different folder name."
                )));
            }
        }

        if partition_size.is_some() && partition.is_none() {
            return Err(CoreError::PermissionError(format!(
                "You can not set partition size without settion partition."
            )));
        }

        let pid = Uuid::new_v4().to_string();
        let data = Buckets {
            id: None,
            pid,
            owner_id,
            owner_type,
            label,
            partition_size,
            partition,
            accepts: accepts.clone(),
            public: public.unwrap_or_default(),
        };

        Buckets::insert(db, &data).await?;

        let bucket = Buckets::get_by_pid(db, &data.pid.clone()).await?;
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(err) = bucket.save_mimes(&db, &accepts).await {
                tracing::error!("unable to save mimes: {err}");
            }
        });

        Ok(data.pid)
    }

    /// save bucket's acceptable mimetypes based on `accepts` parameter.
    async fn save_mimes(&self, db: &RBatis, accepts: &str) -> CoreResult<()> {
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
                    return Err(CoreError::PermissionError(format!(
                        "You need to specify mime list for custom mimetypes."
                    )));
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
                    tracing::error!("{err}")
                }
            }
        }

        Ok(())
    }

    pub async fn validate_mime(&self, db: &RBatis, mime: &str) -> CoreResult<()> {
        if self.accepts == "*" {
            return Ok(());
        }

        let get_mime = Mimes::get_by_key(db, "mime", mime).await?;
        let mime = get_mime.ok_or(CoreError::ServerError(format!(
            "unsupported file mime '{mime}'"
        )))?;

        let bucket_mimes = BucketMimes::select_by_bucket(db, &self.id()).await?;
        let exists = bucket_mimes.iter().find(|bm| *bm.mime_id() == mime.id());

        match exists {
            Some(_) => Ok(()),
            None => Err(CoreError::PermissionError(format!(
                "mime '{}' not supported by selected bucket.",
                mime.mime()
            ))),
        }
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

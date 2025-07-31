use std::path::Path;

use crate::models::de_sqlite_bool;
use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreResult, errors::CoreError, options::CreateBucketOptions, tools::get_folder_size};

#[derive(Serialize, Deserialize, Modeller)]
pub struct Buckets {
    #[modeller(serial)]
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
    accepts: Option<String>,

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
            accepts,
            public: public.unwrap_or_default(),
        };

        Buckets::insert(db, &data).await?;
        Ok(data.pid)
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

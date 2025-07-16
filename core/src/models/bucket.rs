use std::path::Path;

use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CoreResult, errors::CoreError, models::user::Users, options::CreateBucketOptions,
    tools::check_folder_size,
};

#[derive(Serialize, Deserialize, Modeller)]
#[modeller(unique_together(user_id, label))]
pub struct Buckets {
    #[modeller(serial)]
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,

    #[modeller(foreign_key(rf = "users(id)", on_delete = "cascade"))]
    user_id: u64,

    #[modeller(length = 256)]
    label: String,
    partition: Option<String>,

    /// can be set if there's partition
    partition_size: Option<u64>,
    accepts: Option<String>,
}

crud!(Buckets {});

impl_select!(Buckets { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });
impl_select!(Buckets { select_by_pid(user_id: &u64, pid: &str) -> Option => "`WHERE user_id = #{user_id} AND pid = #{pid} LIMIT 1`" });

impl Buckets {
    pub async fn create_by_client(
        db: &RBatis,
        opts: CreateBucketOptions,
    ) -> Result<String, CoreError> {
        let CreateBucketOptions {
            partition_size,
            partition,
            accepts,
            user_id,
            label,
        } = opts;

        let user = Users::get_by_pid(db, &user_id).await?;
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
            user_id: user.id(),
            label,
            partition_size,
            partition,
            accepts,
        };

        Buckets::insert(db, &data).await?;

        let results = Buckets::select_by_map(db, value! { "pid": &data.pid }).await?;
        let id = results
            .first()
            .map(|b| String::from(&b.pid))
            .ok_or(CoreError::ServerError(
                "unable to retrieve created bucker".to_string(),
            ))?;

        Ok(id)
    }

    pub async fn get_by_pid(db: &RBatis, user_id: &u64, pid: &str) -> CoreResult<Self> {
        let s = Self::select_by_pid(db, user_id, pid)
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

            check_folder_size(partition, &mut size).await?;
            // size = Some(folder_size)
        }

        Ok(size)
    }

    pub fn id(&self) -> u64 {
        *&self.id.unwrap_or_default()
    }

    pub fn partition(&self) -> &Option<String> {
        &self.partition
    }

    pub fn partition_size(&self) -> &Option<u64> {
        &self.partition_size
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

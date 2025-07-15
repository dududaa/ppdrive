use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{errors::CoreError, options::CreateBucketOptions};

#[derive(Serialize, Deserialize, Modeller)]
#[modeller(unique_together(owner_id, owner_type))]
pub struct Buckets {
    #[modeller(serial)]
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,

    owner_id: u64,
    owner_type: u8,
    max_size: Option<u64>,
    root_folder: Option<String>,
    accepts: Option<String>,
}

crud!(Buckets {});

impl_select!(Buckets { get_for_owner(owner_id: u64, owner_type: u8) -> Option => "`WHERE owner_id = #{owner_id} AND owner_type = #{owner_type} LIMIT 1`" });
impl_select!(Buckets { get_by_folder(folder: &str) -> Option => "`WHERE root_folder IS LIKE '#{folder}%' LIMIT 1`" });

impl Buckets {
    pub async fn create_by_client(
        db: &RBatis,
        client_id: u64,
        opts: CreateBucketOptions,
    ) -> Result<String, CoreError> {
        let owner_type = BucketOwnerType::Client;
        let owner_type = owner_type.into();
        let pid = Uuid::new_v4().to_string();

        let CreateBucketOptions {
            max_size,
            root_folder,
            accepts,
        } = opts;

        if let Some(folder) = &root_folder {
            let b = Buckets::get_by_folder(db, folder).await?;
            if let Some(b) = b {
                if b.owner_id != client_id && b.owner_type != owner_type {
                    return Err(CoreError::PermissionError(format!(
                        "folder name '{folder}' is not available. Try a different folder name."
                    )));
                }
            }
        }

        let data = Buckets {
            id: None,
            pid,
            owner_id: client_id,
            owner_type,
            max_size,
            root_folder,
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

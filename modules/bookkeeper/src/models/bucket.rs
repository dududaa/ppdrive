use crate::{
    DBResult, Error as AppError,
    models::{
        de_sqlite_bool,
        mime::{BucketMimes, Mimes},
    },
};
use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select};
use rbs::value;
use serde::{Deserialize, Serialize};

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
    pub async fn get_by_pid(db: &RBatis, pid: &str) -> DBResult<Self> {
        let s = Self::get_by_key(db, "pid", pid)
            .await?
            .ok_or(AppError::NotFound("bucket not found".to_string()))?;

        Ok(s)
    }

    pub async fn delete(db: &RBatis, pid: &str) -> DBResult<()> {
        Self::delete_by_map(db, value! { "pid": pid }).await?;
        Ok(())
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

    /// save bucket's acceptable mimetypes based on `accepts` parameter.
    async fn save_mimes(&self, db: &RBatis, accepts: &str) -> DBResult<()> {
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
                    return Err(AppError::PermissionError(format!(
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

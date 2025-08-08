use crate::{DBResult, Error as DBError};
use modeller::prelude::*;
use rbatis::{RBatis, impl_select};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Modeller)]
#[modeller(
    index(name = "idx_mime_filetype", fields(filetype)),
    index(name = "idx_mime", fields(mime))
)]
pub struct Mimes {
    id: Option<u64>,

    #[modeller(length = 60)]
    mime: String,

    #[modeller(length = 20)]
    filetype: String,

    #[modeller(unique, length = 10)]
    label: String,
}

impl_select!(Mimes {});
impl_select!(Mimes { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Mimes {
    pub async fn load_from_file(db: &RBatis) -> DBResult<()> {
        let metalist = include_str!("mimes.json");
        let metalist: Vec<MimeMeta> =
            serde_json::from_str(metalist).map_err(|err| DBError::ParseError(err.to_string()))?;

        for meta in &metalist {
            let MimeMeta {
                mime,
                filetype,
                label,
            } = meta;
            let sql = format!(
                "INSERT INTO mimes (mime, filetype, label) VALUES ({mime}, {filetype}, {label})"
            );
            if let Err(err) = RBatis::exec(db, &sql, vec![]).await {
                println!("{err}")
            }
        }
        Ok(())
    }

    pub fn id(&self) -> u64 {
        *&self.id.unwrap_or_default()
    }

    pub fn mime(&self) -> &str {
        &self.mime
    }
}

/// mime meta to be loaded from json file
#[derive(Deserialize)]
pub struct MimeMeta {
    mime: String,
    filetype: String,
    label: String,
}

#[derive(Serialize, Deserialize, Modeller)]
#[modeller(index(name = "idx_bucket_mimes", fields(bucket_id)))]
pub struct BucketMimes {
    bucket_id: u64,
    mime_id: u64,
}

impl_select!(BucketMimes { select_by_bucket(bucket_id: &u64) => "`WHERE bucket_id = #{bucket_id}`" });

impl BucketMimes {
    pub fn mime_id(&self) -> &u64 {
        &self.mime_id
    }
}

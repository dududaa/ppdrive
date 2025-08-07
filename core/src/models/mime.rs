use modeller::prelude::*;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};

use crate::{CoreResult, errors::CoreError, tools::install_dir};

#[derive(Serialize, Deserialize, Modeller)]
pub struct Mimes {
    id: Option<u64>,

    #[modeller(length = 60)]
    mime: String,

    #[modeller(length = 20)]
    filetype: String,

    #[modeller(unique, length = 10)]
    label: String,
}

impl Mimes {
    pub async fn load_from_file(db: &RBatis) -> CoreResult<()> {
        let filename = "mimes.json";
        let mimepath = if cfg!(debug_assertions) {
            filename.into()
        } else {
            install_dir()?.join(filename)
        };

        let metalist = tokio::fs::read_to_string(mimepath).await?;
        let metalist: Vec<MimeMeta> = serde_json::from_str(&metalist)
            .map_err(|err| CoreError::ParseError(err.to_string()))?;

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
                tracing::error!("{err}")
            }
        }
        Ok(())
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

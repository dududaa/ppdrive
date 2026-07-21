use crate::db::Database;
use crate::{generate_nano_id, sql_safe};
use crate::utils::{asset_owner_id, instance_as_string, AssetOwnerName};

pub struct CreateBucketData {
    size: Option<i64>,
    accepts: Option<Vec<&'static str>>,
    owner_type: AssetOwnerName,
    owner_id: i32,
}

pub async fn create(data: CreateBucketData, db: &Database) -> anyhow::Result<String> {
    let CreateBucketData {
        size,
        accepts,
        owner_type,
        owner_id,
    } = data;
    
    let owner_id = asset_owner_id(owner_type, owner_id, db).await?;
    let pid = generate_nano_id(32);
    let accepts = accepts.map(|s| s.join(","));
    let created_at = instance_as_string()?;

    let mut placeholders = Vec::with_capacity(5);
    for idx in 1..6 {
        placeholders.push(db.placeholder(idx))
    }

    let placeholders = placeholders.join(",");
    let query = sql_safe!("INSERT INTO buckets (pid, size, accepts, created_at, owner_id) VALUES({placeholders})");
    sqlx::query(query).bind(&pid).bind(size).bind(accepts).bind(created_at).bind(owner_id).execute(&**db).await?;
    
    Ok(pid)
}

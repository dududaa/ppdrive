use crate::db::Database;
use crate::utils::{AssetOwnerName, asset_owner_id, instance_as_string};
use crate::{generate_nano_id, sql_safe};

#[derive(Default, Debug)]
pub struct CreateBucketData {
    pub name: String,
    pub path: String,
    pub owner_type: AssetOwnerName,
    pub owner_id: i32,
    pub public: bool,
    pub size: Option<i64>,
    pub accepts: Option<Vec<String>>,
}

pub async fn create(data: &CreateBucketData, db: &Database) -> anyhow::Result<String> {
    let CreateBucketData {
        name,
        public,
        size,
        accepts,
        path,
        owner_type,
        owner_id,
    } = data;

    let owner_id = asset_owner_id(*owner_type, *owner_id, db).await?;
    let pid = generate_nano_id(32);
    let accepts = accepts.as_ref().map(|s| s.join(","));
    let created_at = instance_as_string()?;

    let placeholder_len = 8;
    let mut placeholders = Vec::with_capacity(placeholder_len as usize);
    for idx in 1..placeholder_len+1 {
        placeholders.push(db.placeholder(idx))
    }

    let placeholders = placeholders.join(",");
    let query = sql_safe!(
        "INSERT INTO buckets (pid, size, accepts, created_at, owner_id, path, name, public) VALUES({placeholders})"
    );

    sqlx::query(query)
        .bind(&pid)
        .bind(size)
        .bind(accepts)
        .bind(created_at)
        .bind(owner_id)
        .bind(path)
        .bind(name)
        .bind(public)
        .execute(&**db)
        .await?;

    Ok(pid)
}

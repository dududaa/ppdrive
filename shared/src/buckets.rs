use crate::db::Database;
use crate::utils::{AssetOwnerName, asset_owner_id, instance_as_string};
use crate::{generate_nano_id, sql_safe};
use sqlx::{FromRow, Row};

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
    for idx in 1..placeholder_len + 1 {
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

pub async fn get_id(pid: &str, db: &Database) -> anyhow::Result<i32> {
    let query = sql_safe!(
        "SELECT id FROM buckets WHERE pid = {} LIMIT 1",
        db.placeholder(1)
    );

    let id = sqlx::query_scalar(query).bind(pid).fetch_one(&**db).await?;
    Ok(id)
}

#[derive(FromRow)]
pub struct Bucket {
    pub name: String,
    pub path: String,
    pub public: bool,
    pub size: Option<i64>,
    pub accepts: Option<Vec<String>>,
}

pub async fn get(pid: &str, db: &Database) -> anyhow::Result<Bucket> {
    let query = sql_safe!(
        "SELECT name, path, public, size, accepts FROM buckets WHERE pid = {} LIMIT 1",
        db.placeholder(1)
    );

    let row = sqlx::query(query).bind(pid).fetch_one(&**db).await?;
    let accepts: Option<String> = row.get("name");
    let accepts = accepts.map(|s| s.split(",").map(|s| s.to_string()).collect());

    let data = Bucket {
        name: row.get("name"),
        path: row.get("path"),
        public: row.get("public"),
        size: row.get("size"),
        accepts,
    };

    Ok(data)
}

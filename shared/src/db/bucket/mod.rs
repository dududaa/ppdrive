//! Bucket management — creation, lookup, and parent-path validation.
//!
//! Enforces that private buckets cannot be created inside public buckets
//! and that the requesting entity owns all ancestor paths.

pub mod models;

use crate::db::Database;
use crate::db::utils::{asset_owner_id, instance_as_string};
use crate::{generate_nano_id, sql_safe};
use anyhow::anyhow;
use models::{Bucket, CreateBucketData};
use sqlx::Row;
use std::path::PathBuf;

/// Create a new bucket after validating parent ownership and privacy constraints.
/// Returns the bucket's public PID.
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

    let valid_parents_owner = validate_parent_ownership(path, owner_id, db).await?;
    if !valid_parents_owner {
        return Err(anyhow!(
            "Bucket parent(s) is owned by a different entity. Please choose a another path."
        ));
    }

    let private_parents = validate_parents_privacy(path, db).await?;
    if !public && !private_parents {
        return Err(anyhow!(
            "One or all of the bucket parents is public. This is not allowed for private buckets."
        ));
    }

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

/// Resolve a bucket PID to its numeric ID.
pub async fn get_id(pid: &str, db: &Database) -> anyhow::Result<i32> {
    let query = sql_safe!(
        "SELECT id FROM buckets WHERE pid = {} LIMIT 1",
        db.placeholder(1)
    );

    let id = sqlx::query_scalar(query).bind(pid).fetch_one(&**db).await?;
    Ok(id)
}

/// Fetch a bucket by PID.
pub async fn get(pid: &str, db: &Database) -> anyhow::Result<Bucket> {
    let query = sql_safe!(
        "SELECT name, path, public, size, accepts FROM buckets WHERE pid = {} LIMIT 1",
        db.placeholder(1)
    );

    let row = sqlx::query(query).bind(pid).fetch_one(&**db).await?;
    let accepts: Option<String> = row.get("accepts");
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

/// Check whether any of bucket's parent path is not saved as a public bucket. This is to ensure that the rule
/// **private bucket cannot be created within a public bucket** is not violated.
async fn validate_parents_privacy(path: &str, db: &Database) -> anyhow::Result<bool> {
    let path = PathBuf::from(path);
    let parents = path
        .ancestors()
        .flat_map(|p| p.to_str())
        .collect::<Vec<&str>>();

    let mut placeholders = String::new();
    for idx in 0..parents.len() {
        let placeholder = db.placeholder(idx as u8 + 1);

        if idx == 0 {
            placeholders.push_str(&placeholder);
        } else {
            placeholders.push_str(" OR ");
            placeholders.push_str(&placeholder);
        }
    }

    let query =
        sql_safe!("SELECT Count(*) FROM buckets WHERE path = ({placeholders}) AND public IS TRUE");
    let mut qs = sqlx::query_scalar(query);
    for parent in parents {
        qs = qs.bind(parent);
    }

    let count: i32 = qs.fetch_one(&**db).await?;
    Ok(count == 0)
}

/// Validate that user owns all the parents for this bucket
async fn validate_parent_ownership(
    path: &str,
    owner_id: i32,
    db: &Database,
) -> anyhow::Result<bool> {
    let path = PathBuf::from(path);
    let parents = path
        .ancestors()
        .flat_map(|p| p.to_str())
        .collect::<Vec<&str>>();

    let mut owner_ids: Vec<i32> = Vec::with_capacity(parents.len());
    for parent in parents {
        let query = sql_safe!(
            "SELECT owner_id FROM buckets WHERE path = {}",
            db.placeholder(1)
        );

        let rows = sqlx::query(query).bind(parent).fetch_all(&**db).await?;
        for row in rows {
            owner_ids.push(row.get("owner_id"));
        }
    }

    let mut valid_owner = true;
    for id in owner_ids {
        if id != owner_id {
            valid_owner = false;
            break;
        }
    }

    Ok(valid_owner)
}

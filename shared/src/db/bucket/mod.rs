//! Bucket management — creation, lookup, and parent-path validation.
//!
//! Enforces that private buckets cannot be created inside public buckets
//! and that the requesting entity owns all ancestor paths.

pub mod models;

use crate::db::Database;
use crate::utils::{asset_owner_id, instance_as_string};
use crate::tools::config::StaticFolder;
use crate::{generate_nano_id, paths_cross, sql_safe};
use anyhow::anyhow;
use models::{Bucket, CreateBucketData};
use sqlx::Row;
use std::path::PathBuf;

/// Check if a MIME type matches any entry in an accepts list.
///
/// Exact match for concrete types (e.g. `image/png` matches `image/png`).
/// Prefix match for wildcards (e.g. `image/*` matches `image/png`).
pub fn mime_matches_accepts(mime: &str, accepts: &[String]) -> bool {
    accepts.iter().any(|a| {
        if a.ends_with("/*") {
            let prefix = a.trim_end_matches('*');
            mime.starts_with(prefix)
        } else {
            a.eq_ignore_ascii_case(mime)
        }
    })
}

/// Create a new bucket after validating parent ownership, privacy constraints,
/// and ensuring the path does not cross any existing static folder.
/// Returns the bucket's public PID.
pub async fn create(
    data: &CreateBucketData,
    static_folders: &[StaticFolder],
    db: &Database,
) -> anyhow::Result<String> {
    let CreateBucketData {
        name,
        public,
        size,
        accepts,
        path,
        owner_type,
        owner_id,
    } = data;

    // Normalize path to always have a leading '/' and no trailing '/'.
    let path = path.trim_end_matches('/').to_string();
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };

    for folder in static_folders {
        let default_path = format!("/{}", folder.name);
        let folder_path = folder.path.as_deref().unwrap_or(&default_path);
        if paths_cross(&path, folder_path) {
            return Err(anyhow!(
                "Bucket path '{path}' conflicts with static folder '{}' at '{folder_path}'",
                folder.name
            ));
        }
    }

    let owner_id = asset_owner_id(*owner_type, *owner_id, db).await?;

    let valid_parents_owner = validate_parent_ownership(&path, owner_id, db).await?;
    if !valid_parents_owner {
        return Err(anyhow!(
            "Bucket parent(s) is owned by a different entity. Please choose a another path."
        ));
    }

    let private_parents = validate_parents_privacy(&path, db).await?;
    if !public && !private_parents {
        return Err(anyhow!(
            "One or all of the bucket parents is public. This is not allowed for private buckets."
        ));
    }

    let has_private_parents = validate_parent_privacy_reverse(&path, db).await?;
    if *public && has_private_parents {
        return Err(anyhow!(
            "One or all of the bucket parents is private. This is not allowed for public buckets."
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
        .bind(i32::from(*public))
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
        "SELECT id, name, path, public, size, accepts, owner_id FROM buckets WHERE pid = {} LIMIT 1",
        db.placeholder(1)
    );

    let row = sqlx::query(query).bind(pid).fetch_one(&**db).await?;
    let accepts: Option<String> = row.get("accepts");
    let accepts = accepts.map(|s| s.split(",").map(|s| s.to_string()).collect());

    let data = Bucket {
        id: row.get("id"),
        name: row.get("name"),
        path: row.get("path"),
        public: row.get::<i32, _>("public") != 0,
        size: row.get("size"),
        accepts,
        owner_id: row.get("owner_id"),
    };

    Ok(data)
}

pub async fn get_public_paths(db: &Database) -> anyhow::Result<Vec<String>> {
    let query = sql_safe!("SELECT path FROM buckets WHERE public = 1");
    let result = sqlx::query_scalar(query).fetch_all(&**db).await?;
    Ok(result)
}

/// Fetch all bucket paths from the database.
pub async fn get_all_paths(db: &Database) -> anyhow::Result<Vec<String>> {
    let query = sql_safe!("SELECT path FROM buckets");
    let paths = sqlx::query_scalar(query).fetch_all(&**db).await?;
    Ok(paths)
}

/// Delete a bucket by its public PID.
pub async fn delete_by_pid(pid: &str, db: &Database) -> anyhow::Result<()> {
    let query = sql_safe!("DELETE FROM buckets WHERE pid = {}", db.placeholder(1));
    sqlx::query(query).bind(pid).execute(&**db).await?;
    Ok(())
}

/// Check whether any of bucket's parent path is not saved as a public bucket. This is to ensure that the rule
/// **private bucket cannot be created within a public bucket** is not violated.
async fn validate_parents_privacy(path: &str, db: &Database) -> anyhow::Result<bool> {
    let path = PathBuf::from(path);
    let parents = path
        .ancestors()
        .flat_map(|p| p.to_str())
        .collect::<Vec<&str>>();

    let mut conditions = Vec::with_capacity(parents.len());
    for (idx, _parent) in parents.iter().enumerate() {
        conditions.push(format!("path = {}", db.placeholder(idx as u8 + 1)));
    }
    let where_clause = conditions.join(" OR ");

    let query = sql_safe!(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM buckets WHERE ({where_clause}) AND public = 1) THEN 1 ELSE 0 END"
    );
    let mut qs = sqlx::query_scalar(query);
    for parent in parents {
        qs = qs.bind(parent);
    }

    let exists: i32 = qs.fetch_one(&**db).await?;
    Ok(exists == 0)
}

/// Check whether any of bucket's parent path is saved as a private bucket. This is to ensure that the rule
/// **public bucket cannot be created within a private bucket** is not violated.
async fn validate_parent_privacy_reverse(path: &str, db: &Database) -> anyhow::Result<bool> {
    let path = PathBuf::from(path);
    let parents = path
        .ancestors()
        .flat_map(|p| p.to_str())
        .collect::<Vec<&str>>();

    let mut conditions = Vec::with_capacity(parents.len());
    for (idx, _parent) in parents.iter().enumerate() {
        conditions.push(format!("path = {}", db.placeholder(idx as u8 + 1)));
    }
    let where_clause = conditions.join(" OR ");

    let query = sql_safe!(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM buckets WHERE ({where_clause}) AND public = 0) THEN 1 ELSE 0 END"
    );
    let mut qs = sqlx::query_scalar(query);
    for parent in parents {
        qs = qs.bind(parent);
    }

    let exists: i32 = qs.fetch_one(&**db).await?;
    Ok(exists != 0)
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

    let mut placeholders = String::new();
    for idx in 0..parents.len() {
        let placeholder = db.placeholder(idx as u8 + 1);
        if idx == 0 {
            placeholders.push_str(&placeholder);
        } else {
            placeholders.push_str(", ");
            placeholders.push_str(&placeholder);
        }
    }

    let query = sql_safe!(
        "SELECT DISTINCT owner_id FROM buckets WHERE path IN ({placeholders})",
    );
    let mut qs = sqlx::query_scalar(query);
    for parent in parents {
        qs = qs.bind(parent);
    }

    let owner_ids: Vec<i32> = qs.fetch_all(&**db).await?;

    for id in owner_ids {
        if id != owner_id {
            return Ok(false);
        }
    }

    Ok(true)
}

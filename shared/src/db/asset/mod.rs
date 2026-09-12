use crate::db::Database;
use crate::generate_nano_id;
use crate::sql_safe;
use crate::utils::instance_as_string;
use crate::AssetOwnerName;
use models::{Asset, PermissionLevel, PermissionWithGrantee};

pub mod models;

/// Register a file in the assets table after upload.
pub async fn register(db: &Database, bucket_id: i32, path: &str) -> anyhow::Result<Asset> {
    let pid = generate_nano_id(32);
    let created_at = instance_as_string()?;

    let query = sql_safe!(
        "INSERT INTO assets (pid, bucket_id, path, created_at) VALUES ({}, {}, {}, {}) RETURNING id, pid, bucket_id, path, created_at",
        db.placeholder(1),
        db.placeholder(2),
        db.placeholder(3),
        db.placeholder(4)
    );

    let asset = sqlx::query_as::<_, Asset>(query)
        .bind(&pid)
        .bind(bucket_id)
        .bind(path)
        .bind(&created_at)
        .fetch_one(&**db)
        .await?;

    Ok(asset)
}

/// Get an asset by PID.
pub async fn get_by_pid(db: &Database, pid: &str) -> anyhow::Result<Asset> {
    let query = sql_safe!(
        "SELECT id, pid, bucket_id, path, created_at FROM assets WHERE pid = {} LIMIT 1",
        db.placeholder(1)
    );

    let asset = sqlx::query_as::<_, Asset>(query)
        .bind(pid)
        .fetch_one(&**db)
        .await?;

    Ok(asset)
}

/// Get an asset by bucket ID and relative path.
pub async fn get_by_bucket_and_path(
    db: &Database,
    bucket_id: i32,
    path: &str,
) -> anyhow::Result<Option<Asset>> {
    let query = sql_safe!(
        "SELECT id, pid, bucket_id, path, created_at FROM assets WHERE bucket_id = {} AND path = {} LIMIT 1",
        db.placeholder(1),
        db.placeholder(2)
    );

    let asset = sqlx::query_as::<_, Asset>(query)
        .bind(bucket_id)
        .bind(path)
        .fetch_optional(&**db)
        .await?;

    Ok(asset)
}

/// Grant or update a permission on an asset for a grantee.
pub async fn grant(
    db: &Database,
    asset_id: i32,
    grantee_owner_id: i32,
    permission: PermissionLevel,
) -> anyhow::Result<()> {
    let created_at = instance_as_string()?;
    let perm: i16 = permission.into();

    let query = sql_safe!(
        "INSERT INTO file_permissions (asset_id, grantee_id, permission, created_at) VALUES ({}, {}, {}, {}) ON CONFLICT (asset_id, grantee_id) DO UPDATE SET permission = {}",
        db.placeholder(1),
        db.placeholder(2),
        db.placeholder(3),
        db.placeholder(4),
        db.placeholder(5)
    );

    sqlx::query(query)
        .bind(asset_id)
        .bind(grantee_owner_id)
        .bind(perm)
        .bind(&created_at)
        .bind(perm)
        .execute(&**db)
        .await?;

    Ok(())
}

/// Revoke a permission on an asset for a grantee.
pub async fn revoke(db: &Database, asset_id: i32, grantee_owner_id: i32) -> anyhow::Result<()> {
    let query = sql_safe!(
        "DELETE FROM file_permissions WHERE asset_id = {} AND grantee_id = {}",
        db.placeholder(1),
        db.placeholder(2)
    );

    sqlx::query(query)
        .bind(asset_id)
        .bind(grantee_owner_id)
        .execute(&**db)
        .await?;

    Ok(())
}

/// List all permissions for an asset, resolving grantee name from either clients or users.
pub async fn list_permissions(db: &Database, asset_id: i32) -> anyhow::Result<Vec<PermissionWithGrantee>> {
    let query = sql_safe!(
        "SELECT fp.id, fp.asset_id, fp.grantee_id, ao.name AS grantee_type,
                COALESCE(c.name, u.email) AS grantee_name,
                fp.permission, fp.created_at
         FROM file_permissions fp
         JOIN asset_owner ao ON fp.grantee_id = ao.id
         LEFT JOIN clients c ON ao.name = {} AND ao.owner_id = c.id
         LEFT JOIN users u ON ao.name = {} AND ao.owner_id = u.id
         WHERE fp.asset_id = {}",
        db.placeholder(1),
        db.placeholder(2),
        db.placeholder(3)
    );

    let rows: Vec<(i32, i32, i32, i16, String, i16, String)> = sqlx::query_as(query)
        .bind(i16::from(AssetOwnerName::Client))
        .bind(i16::from(AssetOwnerName::User))
        .bind(asset_id)
        .fetch_all(&**db)
        .await?;

    let permissions = rows
        .into_iter()
        .map(|(id, asset_id, grantee_id, grantee_type, grantee_name, perm, created_at)| {
            let level = PermissionLevel::from(perm);
            let grantee_type = if grantee_type == i16::from(AssetOwnerName::User) {
                "user".to_string()
            } else {
                "client".to_string()
            };
            PermissionWithGrantee {
                id,
                asset_id,
                grantee_id,
                grantee_type,
                grantee_name,
                permission: level.to_string(),
                created_at,
            }
        })
        .collect();

    Ok(permissions)
}

/// List all permissions for all assets in a bucket.
pub async fn list_all_permissions_for_bucket(db: &Database, bucket_id: i32) -> anyhow::Result<Vec<PermissionWithGrantee>> {
    let query = sql_safe!(
        "SELECT fp.id, fp.asset_id, fp.grantee_id, ao.name AS grantee_type,
                COALESCE(c.name, u.email) AS grantee_name,
                fp.permission, fp.created_at
         FROM file_permissions fp
         JOIN assets a ON fp.asset_id = a.id
         JOIN asset_owner ao ON fp.grantee_id = ao.id
         LEFT JOIN clients c ON ao.name = {} AND ao.owner_id = c.id
         LEFT JOIN users u ON ao.name = {} AND ao.owner_id = u.id
         WHERE a.bucket_id = {}",
        db.placeholder(1),
        db.placeholder(2),
        db.placeholder(3)
    );

    let rows: Vec<(i32, i32, i32, i16, String, i16, String)> = sqlx::query_as(query)
        .bind(i16::from(AssetOwnerName::Client))
        .bind(i16::from(AssetOwnerName::User))
        .bind(bucket_id)
        .fetch_all(&**db)
        .await?;

    let permissions = rows
        .into_iter()
        .map(|(id, asset_id, grantee_id, grantee_type, grantee_name, perm, created_at)| {
            let level = PermissionLevel::from(perm);
            let grantee_type = if grantee_type == i16::from(AssetOwnerName::User) {
                "user".to_string()
            } else {
                "client".to_string()
            };
            PermissionWithGrantee {
                id,
                asset_id,
                grantee_id,
                grantee_type,
                grantee_name,
                permission: level.to_string(),
                created_at,
            }
        })
        .collect();

    Ok(permissions)
}

/// Check if a grantee has at least the specified permission level on an asset.
pub async fn has_permission(
    db: &Database,
    asset_id: i32,
    grantee_owner_id: i32,
    min_permission: PermissionLevel,
) -> anyhow::Result<bool> {
    let min_perm: i16 = min_permission.into();

    let query = sql_safe!(
        "SELECT CASE WHEN EXISTS(SELECT 1 FROM file_permissions WHERE asset_id = {} AND grantee_id = {} AND permission >= {}) THEN 1 ELSE 0 END",
        db.placeholder(1),
        db.placeholder(2),
        db.placeholder(3)
    );

    let exists: i32 = sqlx::query_scalar(query)
        .bind(asset_id)
        .bind(grantee_owner_id)
        .bind(min_perm)
        .fetch_one(&**db)
        .await?;

    Ok(exists != 0)
}

use crate::db::Database;
use crate::sql_safe;

/// Count of all registered users.
pub async fn count_users(db: &Database) -> anyhow::Result<i64> {
    let query = sql_safe!("SELECT COUNT(*) FROM users");
    let count: i64 = sqlx::query_scalar(query).fetch_one(&**db).await?;
    Ok(count)
}

/// Count of users created after the given RFC 3339 timestamp.
pub async fn count_users_after(db: &Database, since: &str) -> anyhow::Result<i64> {
    let query = sql_safe!(
        "SELECT COUNT(*) FROM users WHERE created_at >= {}",
        db.placeholder(1)
    );
    let count: i64 = sqlx::query_scalar(query).bind(since).fetch_one(&**db).await?;
    Ok(count)
}

/// Count of all clients.
pub async fn count_clients(db: &Database) -> anyhow::Result<i64> {
    let query = sql_safe!("SELECT COUNT(*) FROM clients");
    let count: i64 = sqlx::query_scalar(query).fetch_one(&**db).await?;
    Ok(count)
}

/// Count of all assets (files).
pub async fn count_assets(db: &Database) -> anyhow::Result<i64> {
    let query = sql_safe!("SELECT COUNT(*) FROM assets");
    let count: i64 = sqlx::query_scalar(query).fetch_one(&**db).await?;
    Ok(count)
}

/// Count of all buckets.
pub async fn count_buckets(db: &Database) -> anyhow::Result<i64> {
    let query = sql_safe!("SELECT COUNT(*) FROM buckets");
    let count: i64 = sqlx::query_scalar(query).fetch_one(&**db).await?;
    Ok(count)
}

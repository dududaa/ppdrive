//! User management — creation, lookup, and asset-owner registration.

use crate::db::Database;
use crate::sql_safe;
use crate::utils::{check_password, instance_as_string, AssetOwnerName, make_password};

/// Create a new user, hash the password with Argon2, and register as an asset owner.
pub async fn create(email: &str, password: &str, db: &Database) -> anyhow::Result<()> {
    let password = make_password(password)?;
    let now = instance_as_string()?;

    let mut placeholders = Vec::with_capacity(3);
    for idx in 1..4 {
        placeholders.push(db.placeholder(idx))
    }

    let placeholders = placeholders.join(",");
    let query =
        sql_safe!("INSERT INTO users (email, password, created_at, updated_at) VALUES ({placeholders}, {})", db.placeholder(4));

    sqlx::query(query)
        .bind(email)
        .bind(&password)
        .bind(&now)
        .bind(&now)
        .execute(&**db)
        .await?;

    let query = sql_safe!("SELECT id FROM users WHERE email = {}", db.placeholder(1));
    let id: i32 = sqlx::query_scalar(query)
        .bind(email)
        .fetch_one(&**db)
        .await?;

    let query = sql_safe!(
        "INSERT INTO asset_owner(name, owner_id) VALUES({}, {})",
        db.placeholder(1),
        db.placeholder(2)
    );

    sqlx::query(query)
        .bind(i16::from(AssetOwnerName::User))
        .bind(id)
        .execute(&**db)
        .await?;

    Ok(())
}

/// Resolve a user email to its numeric ID.
pub async fn get_id(email: &str, db: &Database) -> anyhow::Result<i32> {
    let query = sql_safe!(
        "SELECT id FROM users WHERE email = {} LIMIT 1",
        db.placeholder(1)
    );
    let id: i32 = sqlx::query_scalar(query)
        .bind(email)
        .fetch_one(&**db)
        .await?;
    Ok(id)
}

/// Find a user by email, returning the hashed password.
pub async fn find_by_email(email: &str, db: &Database) -> anyhow::Result<(i32, String)> {
    let query = sql_safe!(
        "SELECT id, password FROM users WHERE email = {} LIMIT 1",
        db.placeholder(1)
    );
    let row: (i32, String) = sqlx::query_as(query)
        .bind(email)
        .fetch_one(&**db)
        .await?;
    Ok(row)
}

/// Verify a user's password against the stored hash.
pub async fn verify_password(email: &str, password: &str, db: &Database) -> anyhow::Result<i32> {
    let (id, hashed) = find_by_email(email, db).await?;
    check_password(password, &hashed)?;
    Ok(id)
}

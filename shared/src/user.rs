use crate::db::Database;
use crate::server::make_password;
use crate::{sql_safe};
use crate::utils::{instance_as_string, AssetOwnerName};

pub async fn create(email: &str, password: &str, db: &Database) -> anyhow::Result<()> {
    let password = make_password(password);
    let now = instance_as_string()?;

    let mut placeholders = Vec::with_capacity(3);
    for idx in 1..4 {
        placeholders.push(db.placeholder(idx))
    }

    let placeholders = placeholders.join(",");
    let query =
        sql_safe!("INSERT INTO users (email, password, created_at) VALUES ({placeholders})");
    
    sqlx::query(query)
        .bind(email)
        .bind(password)
        .bind(now)
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

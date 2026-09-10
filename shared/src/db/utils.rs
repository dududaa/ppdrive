//! Database utility types and helpers.
//!
//! Provides the [`AssetOwnerName`] enum, [`sql_safe!`] macro for engine-agnostic
//! placeholder interpolation, and ownership-checking queries.

/// Utilities used by database queries
// use crate::sql_safe;
use crate::db::Database;
use clap::ValueEnum;
use time::OffsetDateTime;

#[macro_export]
macro_rules! sql_safe {
    ($($arg:tt)*) => {{
        let query = format!($($arg)*);
        let sql = $crate::db::utils::SqlSafe::new(query);

        sql.into_inner()
    }};
}

/// Resolve an [`AssetOwnerName`] + numeric owner ID to the `asset_owner.id` primary key.
pub async fn asset_owner_id(
    owner_name: AssetOwnerName,
    owner_id: i32,
    db: &Database,
) -> anyhow::Result<i32> {
    let query = sql_safe!(
        "SELECT id FROM asset_owner WHERE name = {} AND owner_id = {}",
        db.placeholder(1),
        db.placeholder(2)
    );
    
    let id = sqlx::query_scalar(query)
        .bind(i16::from(owner_name))
        .bind(owner_id)
        .fetch_one(&**db)
        .await?;

    Ok(id)
}

/// The type of entity that owns a bucket or other asset (User or Client).
#[derive(ValueEnum, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetOwnerName {
    User,
    #[default]
    Client,
}

impl From<i16> for AssetOwnerName {
    fn from(value: i16) -> Self {
        use AssetOwnerName::*;

        match value {
            0 => User,
            1 => Client,
            _ => Default::default(),
        }
    }
}

impl From<AssetOwnerName> for i16 {
    fn from(value: AssetOwnerName) -> Self {
        use AssetOwnerName::*;

        match value {
            User => 0,
            Client => 1,
        }
    }
}

/// Wrapper that asserts a SQL string is safe for direct interpolation (no user input).
pub struct SqlSafe<T> {
    inner: sqlx::AssertSqlSafe<T>,
}

impl<T> SqlSafe<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: sqlx::AssertSqlSafe(value),
        }
    }

    pub fn into_inner(self) -> sqlx::AssertSqlSafe<T> {
        self.inner
    }
}

/// Return the current UTC time formatted as RFC 3339.
pub fn instance_as_string() -> anyhow::Result<String> {
    let now = OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;
    Ok(now)
}

/// Check whether an asset owner entry exists for the given type and numeric ID.
pub async fn check_ownership(owner_type: AssetOwnerName, owner_id: i32, db: &Database) -> anyhow::Result<bool> {
    let query = sql_safe!("SELECT EXISTS(SELECT 1 FROM asset_owner WHERE name = {} AND owner_id = {})", db.placeholder(1), db.placeholder(2));
    let exists: bool = sqlx::query_scalar(query).bind(i16::from(owner_type)).bind(owner_id).fetch_one(&**db).await?;

    Ok(exists)
}
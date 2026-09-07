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

pub fn instance_as_string() -> anyhow::Result<String> {
    let now = OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;
    Ok(now)
}

pub async fn check_ownership(owner_type: AssetOwnerName, owner_id: i32, db: &Database) -> anyhow::Result<bool> {
    let query = sql_safe!("SELECT Count(*) FROM asset_owner WHERE name = {} AND owner_id = {} LIMIT 1", db.placeholder(1), db.placeholder(2));
    let  count: i32 = sqlx::query_scalar(query).bind(i16::from(owner_type)).bind(owner_id).fetch_one(&**db).await?;
    
    Ok(count > 0)
}
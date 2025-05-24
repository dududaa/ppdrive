use rbatis::RBatis;
use rbdc_mssql::MssqlDriver;
use rbdc_mysql::MysqlDriver;
use rbdc_pg::PgDriver;
use rbdc_sqlite::SqliteDriver;
use sqlx::any::{AnyPoolOptions, install_default_drivers};

use crate::{CoreResult, errors::CoreError};

pub async fn init_db(url: &str) -> CoreResult<RBatis> {
    use DatabaseType::*;

    if !cfg!(debug_assertions) {
        migrate(url).await?;
    }

    let db_type = url.try_into()?;
    let rb = RBatis::new();

    match db_type {
        Sqlite => rb.init(SqliteDriver {}, url)?,
        MySql => rb.init(MysqlDriver {}, url)?,
        Postgres => rb.init(PgDriver {}, url)?,
        MsSql => rb.init(MssqlDriver {}, url)?,
    }

    Ok(rb)
}

enum DatabaseType {
    MySql,
    Postgres,
    Sqlite,
    MsSql,
}

impl<'a> TryFrom<&'a str> for DatabaseType {
    type Error = CoreError;

    fn try_from(url: &'a str) -> Result<Self, Self::Error> {
        use DatabaseType::*;

        if url.starts_with("mysql") {
            Ok(MySql)
        } else if url.starts_with("postgres") {
            Ok(Postgres)
        } else if url.starts_with("sqlite") {
            Ok(Sqlite)
        } else if url.starts_with("mssql") {
            Ok(MsSql)
        } else {
            Err(CoreError::ParseError(
                "unsupported database type".to_string(),
            ))
        }
    }
}

async fn migrate(url: &str) -> CoreResult<()> {
    install_default_drivers();
    let pool = AnyPoolOptions::new().connect(url).await?;
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(|err| CoreError::MigrationError(err.into()))?;

    Ok(())
}

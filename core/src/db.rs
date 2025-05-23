use rbatis::RBatis;
use rbdc_mssql::MssqlDriver;
use rbdc_mysql::MysqlDriver;
use rbdc_pg::PgDriver;
use rbdc_sqlite::SqliteDriver;

use crate::{CoreResult, errors::CoreError};

pub fn init_db(url: &str) -> CoreResult<RBatis> {
    use DatabaseType::*;
    fast_log::init(fast_log::Config::new().console()).expect("rbatis init fail");
    let db_type = url.try_into()?;

    let rb = RBatis::new();
    match db_type {
        Sqlite => rb.init(SqliteDriver {}, url)?,
        MySql => rb.init(MysqlDriver {}, url)?,
        Postgres => rb.init(PgDriver {}, url)?,
    }

    Ok(rb)
}

enum DatabaseType {
    MySql,
    Postgres,
    Sqlite,
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
        } else {
            Err(CoreError::ParseError(
                "unsupported database type".to_string(),
            ))
        }
    }
}

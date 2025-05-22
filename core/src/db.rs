use rbatis::RBatis;
use rbdc_mssql::MssqlDriver;
use rbdc_mysql::MysqlDriver;
use rbdc_pg::PgDriver;
use rbdc_sqlite::SqliteDriver;

use crate::{CoreResult, errors::CoreError};

pub fn init_db(db_type: &str, url: &str) -> CoreResult<RBatis> {
    use DatabaseType::*;
    fast_log::init(fast_log::Config::new().console()).expect("rbatis init fail");
    let db_type = db_type.try_into()?;

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

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        use DatabaseType::*;

        match value {
            "mysql" => Ok(MySql),
            "postgres" => Ok(Postgres),
            "sqlite" => Ok(Sqlite),
            "mssql" => Ok(MsSql),
            _ => Err(CoreError::ParseError("unsupported db_type".to_string())),
        }
    }
}

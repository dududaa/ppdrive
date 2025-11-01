use std::str::FromStr;

use rbatis::RBatis;
use rbdc_mssql::MssqlDriver;
use rbdc_mysql::MysqlDriver;
use rbdc_pg::PgDriver;
use rbdc_sqlite::SqliteDriver;

use crate::{db::migration::run_migrations, errors::Error, models::mime::Mimes, DBResult};

pub mod migration;

pub async fn init_db(url: &str, load_mimes: bool) -> DBResult<RBatis> {
    use DatabaseType::*;

    let db_type = url.parse()?;
    let rb = RBatis::new();
    
    match db_type {
        Sqlite => rb.init(SqliteDriver {}, url)?,
        MySql => rb.init(MysqlDriver {}, url)?,
        Postgres => rb.init(PgDriver {}, url)?,
        MsSql => rb.init(MssqlDriver {}, url)?,
    }

    // load mimes in the background
    let db_clone = rb.clone();
    if load_mimes  && let Err(err) = Mimes::load_from_file(&db_clone).await {
        tracing::warn!("{err}")
    }
    
    run_migrations(url).await?;
    Ok(rb)
}

enum DatabaseType {
    MySql,
    Postgres,
    Sqlite,
    MsSql,
}

impl FromStr for DatabaseType {
    type Err = Error;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
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
            Err(Error::ParseError("unsupported database type".to_string()))
        }
    }
}

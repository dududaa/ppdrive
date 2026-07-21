pub mod broker;
pub mod client;
pub mod db;
mod tools;

#[cfg(feature = "server")]
pub mod server;

use std::ops::Deref;
pub use tools::*;

#[derive(Default)]
pub enum AssetOwner {
    User,
    #[default]
    Client
}

impl From<i16> for AssetOwner {
    fn from(value: i16) -> Self {
        use AssetOwner::*;
        
        match value { 
            0 => User,
            1 => Client,
            _ => Default::default()
        }
    }
}

impl From<AssetOwner> for i16 {
    fn from(value: AssetOwner) -> Self {
        use AssetOwner::*;
        
        match value { 
            User => 0,
            Client => 1,
        }
    }
}

pub struct SqlSafe<T> {
    inner: sqlx::AssertSqlSafe<T>
}

impl<T> SqlSafe<T> {
    pub fn new(value: T) -> Self {
        Self{ inner: sqlx::AssertSqlSafe(value) }
    }
    
    pub fn into_inner(self) -> sqlx::AssertSqlSafe<T> {
        self.inner
    }
}


#[macro_export]
macro_rules! sql_safe {
    ($($arg:tt)*) => {{
        let query = format!($($arg)*);
        let sql = SqlSafe::new(query);
        
        sql.into_inner()
    }};
}
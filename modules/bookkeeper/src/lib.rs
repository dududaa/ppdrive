pub use errors::Error;
pub use rbatis::RBatis;

pub mod db;
pub mod models;
type DBResult<T> = Result<T, Error>;

pub mod errors;
pub mod validators;

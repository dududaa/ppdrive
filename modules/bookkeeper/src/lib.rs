use errors::Error;

pub mod db;
pub mod models;
type DBResult<T> = Result<T, Error>;

pub mod errors;

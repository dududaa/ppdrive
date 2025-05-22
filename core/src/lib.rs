use errors::CoreError;
pub use rbatis::RBatis;

pub mod db;
pub mod errors;
mod fs;
pub mod models;
pub mod options;

pub(self) type CoreResult<T> = Result<T, CoreError>;

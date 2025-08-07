use errors::CoreError;
pub use rbatis::RBatis;
pub use toml;

pub mod config;
pub mod db;
pub mod errors;
mod fs;
pub mod models;
pub mod options;
pub mod tools;

pub(self) type CoreResult<T> = Result<T, CoreError>;

#[cfg(feature = "auth")]
pub mod auth;
mod errors;
pub mod free;
pub mod opts;
pub mod utils;

pub type FsResult<T> = Result<T, crate::errors::Error>;

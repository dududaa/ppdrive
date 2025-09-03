use crate::errors::Error;
pub mod errors;
pub mod tools;
pub mod plugin;
pub mod opts;

pub type AppResult<T> = Result<T, Error>;
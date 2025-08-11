use crate::errors::Error;
pub mod config;
pub mod errors;
pub mod tools;

pub type AppResult<T> = Result<T, Error>;
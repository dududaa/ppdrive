pub mod fs;
pub mod jwt;
pub mod sqlx;
pub mod tools;

use crate::errors::AppError;

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

pub fn mb_to_bytes(value: usize) -> usize {
    value * 1024 * 1000
}

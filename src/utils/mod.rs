pub mod sqlx_ext;
pub mod sqlx_utils;
pub mod tools;

use crate::errors::AppError;

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

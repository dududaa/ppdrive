use crate::errors::AppError;
use ppdrive_core::tools::secrets::{generate_secret_file, secret_filename};

pub mod fs;
pub mod jwt;

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

pub fn mb_to_bytes(value: usize) -> usize {
    value * 1024 * 1000
}

/// If app secret file does not exist, generate it. Mostly useful
/// on app initialization.
pub async fn init_secrets() -> Result<(), AppError> {
    let path = secret_filename()?;
    if !path.is_file() {
        generate_secret_file().await?;
    }

    Ok(())
}

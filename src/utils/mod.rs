use crate::errors::PPDriveError;

pub fn get_env(key: &str) -> Result<String, PPDriveError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}
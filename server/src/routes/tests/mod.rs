use std::{path::PathBuf, str::FromStr};

use ppdrive_core::{
    config::AppConfig,
    tools::{create_client, secrets::AppSecrets},
    RBatis,
};

use crate::{errors::AppError, AppResult};

mod client;

async fn app_config() -> AppResult<AppConfig> {
    let config_path = PathBuf::from_str("../ppd_config.toml")
        .map_err(|err| AppError::InternalServerError(err.to_string()))?;
    let config = AppConfig::load(config_path).await?;
    Ok(config)
}

async fn client_token(db: &RBatis) -> AppResult<String> {
    let secrets = AppSecrets::read().await?;
    let token = create_client(&db, &secrets, "Test Client").await?;

    Ok(token)
}

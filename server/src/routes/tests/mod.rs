use std::{path::PathBuf, str::FromStr};

use axum_test::TestServer;
use ppdrive_core::{
    config::AppConfig,
    db::init_db,
    tools::{create_client, secrets::AppSecrets},
    RBatis,
};

use crate::{app::initialize_app, errors::AppError, AppResult};

mod client;

async fn app_config() -> AppResult<AppConfig> {
    let config_path = PathBuf::from_str("../ppd_config.toml")
        .map_err(|err| AppError::InternalServerError(err.to_string()))?;
    let config = AppConfig::load(config_path).await?;
    Ok(config)
}

async fn create_client_token(db: &RBatis) -> AppResult<String> {
    let secrets = AppSecrets::read().await?;
    let token = create_client(&db, &secrets, "Test Client").await?;

    Ok(token)
}

async fn create_server(config: &AppConfig) -> AppResult<TestServer> {
    let app = initialize_app(&config).await?;
    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    Ok(server)
}

async fn create_db(config: &AppConfig) -> AppResult<RBatis> {
    let url = config.db().url();
    let db = init_db(url).await?;

    Ok(db)
}

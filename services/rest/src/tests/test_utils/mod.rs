use axum_test::TestServer;
use ppd_bk::{db::init_db, RBatis};
use ppd_shared::{
    config::AppConfig,
    tools::AppSecrets,
};
use client_tools::{create_client, errors::Error as ClientError};

use crate::{initialize_app, errors::ServerError, ServerResult};

pub mod functions;

const HEADER_NAME: &str = "x-ppd-client";

pub async fn app_config() -> ServerResult<AppConfig> {
    let config = AppConfig::load().await?;
    Ok(config)
}

pub async fn create_client_token(db: &RBatis) -> ServerResult<String> {
    let secrets = AppSecrets::read().await?;
    let token = create_client(&db, &secrets, "Test Client").await?;

    Ok(token)
}

#[allow(dead_code)]
pub async fn create_server(config: &AppConfig) -> ServerResult<TestServer> {
    let app = initialize_app(&config).await?;
    let server = TestServer::new(app).map_err(|err| {
        ServerError::InternalError(format!("unable to create test server: {err}"))
    })?;

    Ok(server)
}

#[allow(dead_code)]
pub async fn create_db(config: &AppConfig) -> ServerResult<RBatis> {
    let url = config.db().url();
    let db = init_db(url).await?;

    Ok(db)
}

impl From<ClientError> for ServerError {
    fn from(value: ClientError) -> Self {
        ServerError::InternalError(value.to_string())
    }
}
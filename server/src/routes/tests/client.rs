use std::{path::PathBuf, str::FromStr};

use axum_test::TestServer;
use ppdrive_core::{
    config::AppConfig,
    db::init_db,
    models::bucket::Buckets,
    options::CreateBucketOptions,
    tools::{create_client, secrets::AppSecrets},
};

use crate::{app::initialize_app, errors::AppError, AppResult};

#[tokio::test]
async fn test_create_bucket() -> AppResult<()> {
    let config_path = PathBuf::from_str("../ppd_config.toml")
        .map_err(|err| AppError::InternalServerError(err.to_string()))?;
    let config = AppConfig::load(config_path).await?;
    let app = initialize_app(&config).await?;

    let url = config.db().url();
    let db = init_db(url).await?;
    let secrets = AppSecrets::read().await?;

    let client_token = create_client(&db, &secrets, "Test Client").await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let opts = CreateBucketOptions::default();
    let resp = server
        .post("/client/bucket")
        .json(&opts)
        .add_header("x-ppd-client", client_token)
        .await;

    // clean up
    let pid = resp.text();
    Buckets::delete(&db, &pid).await?;

    resp.assert_status_ok();

    Ok(())
}

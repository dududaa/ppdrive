use axum_test::TestServer;
use ppdrive_core::{
    db::init_db,
    models::{bucket::Buckets, user::UserRole},
    options::{CreateBucketOptions, CreateUserOptions},
};

use crate::{
    app::initialize_app,
    errors::AppError,
    routes::tests::{app_config, client_token},
    AppResult,
};

const HEADER_NAME: &str = "x-ppd-client";

#[tokio::test]
async fn test_create_user() -> AppResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let data = CreateUserOptions {
        role: UserRole::Basic,
    };

    let resp = server
        .post("/client/user/register")
        .json(&data)
        .add_header(HEADER_NAME, token)
        .await;

    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
async fn test_create_bucket() -> AppResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let opts = CreateBucketOptions::default();
    let resp = server
        .post("/client/bucket")
        .json(&opts)
        .add_header(HEADER_NAME, token)
        .await;

    // clean up
    let pid = resp.text();
    Buckets::delete(&db, &pid).await?;

    resp.assert_status_ok();

    Ok(())
}

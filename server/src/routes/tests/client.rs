use axum_test::{TestResponse, TestServer};
use ppdrive_core::{
    db::init_db,
    options::{CreateBucketOptions, CreateUserClient, LoginUserClient},
};
use serial_test::serial;

use crate::{
    app::initialize_app,
    errors::AppError,
    routes::tests::{app_config, client_token},
    AppResult,
};

const HEADER_NAME: &str = "x-ppd-client";

async fn create_user_request(server: &TestServer, token: &str) -> TestResponse {
    let data = CreateUserClient { max_bucket: None };
    server
        .post("/client/user/register")
        .json(&data)
        .add_header(HEADER_NAME, token)
        .await
}

#[tokio::test]
#[serial]
async fn test_client_create_user() -> AppResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let resp = create_user_request(&server, &token).await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_login_user() -> AppResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let resp = create_user_request(&server, &token).await;
    let user_id = resp.text();

    let data = LoginUserClient {
        id: user_id,
        access_exp: None,
        refresh_exp: None,
    };

    let resp = server
        .post("/client/user/login")
        .add_header("x-ppd-client", token)
        .json(&data)
        .await;

    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_delete_user() -> AppResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let resp = create_user_request(&server, &token).await;
    let user_id = resp.text();

    let resp = server
        .delete(&format!("/client/user/{user_id}"))
        .add_header("x-ppd-client", token)
        .await;

    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_create_bucket() -> AppResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        AppError::InternalServerError(format!("unable to create test server: {err}"))
    })?;

    let opts = CreateBucketOptions {
        ..Default::default()
    };

    let resp = server
        .post("/client/bucket")
        .json(&opts)
        .add_header(HEADER_NAME, token)
        .await;

    resp.assert_status_ok();

    Ok(())
}

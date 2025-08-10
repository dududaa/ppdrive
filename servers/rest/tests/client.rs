use axum_test::TestServer;
use ppd_bk::db::init_db;
use serial_test::serial;

use ppd_rest::{errors::ServerError, initialize_app, ServerResult};
use test_utils::{app_config, create_client_token, functions::{create_user_request, login_user_request, create_client_bucket}};

mod test_utils;

#[tokio::test]
#[serial]
/// create user by a client
async fn test_client_create_user() -> ServerResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = create_client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        ServerError::InternalError(format!("unable to create test server: {err}"))
    })?;

    let resp = create_user_request(&server, &token).await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_login_user() -> ServerResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = create_client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        ServerError::InternalError(format!("unable to create test server: {err}"))
    })?;

    let resp = login_user_request(&server, &token).await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_delete_user() -> ServerResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = create_client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        ServerError::InternalError(format!("unable to create test server: {err}"))
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
async fn test_client_create_bucket() -> ServerResult<()> {
    let config = app_config().await?;

    let url = config.db().url();
    let db = init_db(url).await?;

    let token = create_client_token(&db).await?;
    let app = initialize_app(&config).await?;

    let server = TestServer::new(app).map_err(|err| {
        ServerError::InternalError(format!("unable to create test server: {err}"))
    })?;

    let resp = create_client_bucket(&server, &token).await;
    resp.assert_status_ok();

    Ok(())
}

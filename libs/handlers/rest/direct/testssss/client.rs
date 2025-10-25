use serial_test::serial;

use crate::test_utils::{
    functions::{create_client_bucket, create_user_request, login_user_request}, TestApp
};

mod test_utils;

#[tokio::test]
#[serial]
/// create user by a client
async fn test_client_create_user() {
    let app = TestApp::new().await;
    let token = app.client_token().await;

    let server = app.server();
    let resp = create_user_request(&server, &token).await;

    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_client_login_user() {
    let app = TestApp::new().await;
    let token = app.client_token().await;

    let server = app.server();
    let resp = login_user_request(&server, &token).await;

    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_client_delete_user() {
    let app = TestApp::new().await;
    let token = app.client_token().await;

    let server = app.server();
    let resp = create_user_request(&server, &token).await;

    let user_id = resp.text();
    let resp = server
        .delete(&format!("/client/user/{user_id}"))
        .add_header("x-ppd-client", token)
        .await;

    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_client_create_bucket() {
    let app = TestApp::new().await;
    let token = app.client_token().await;

    let server = app.server();

    let resp = create_client_bucket(&server, &token).await;
    resp.assert_status_ok();
}

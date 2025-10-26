use serial_test::serial;

use rest_test_utils::{
    client::{create_client_bucket, create_user_request, login_user_request}, TestApp, HEADER_TOKEN_KEY
};

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
    let user_id = create_user_request(&server, &token).await.text();

    let resp = server
        .delete(&format!("/client/user/{user_id}"))
        .add_header(HEADER_TOKEN_KEY, token)
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

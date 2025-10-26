use axum_test::{TestResponse, TestServer};
use ppdrive::prelude::opts::{CreateUserClient, LoginUserClient};
use ppd_bk::models::bucket::CreateBucketOptions;

use super::HEADER_NAME;

#[allow(dead_code)]
pub async fn create_user_bucket(server: &TestServer, token: &str) -> TestResponse {
    let opts = CreateBucketOptions::default();

    server
        .post("/client/user/bucket")
        .json(&opts)
        .authorization_bearer(token)
        .await
}

pub async fn create_user_request(server: &TestServer, token: &str) -> TestResponse {
    let data = CreateUserClient { max_bucket: None };
    server
        .post("/client/user/register")
        .json(&data)
        .add_header(HEADER_NAME, token)
        .await
}

pub async fn login_user_request(server: &TestServer, token: &str) -> TestResponse {
    let resp = create_user_request(&server, &token).await;
    let user_id = resp.text();

    let data = LoginUserClient {
        id: user_id,
        access_exp: None,
        refresh_exp: None,
    };

    server
        .post("/client/user/login")
        .add_header("x-ppd-client", token)
        .json(&data)
        .await
}

#[allow(dead_code)]
pub async fn create_client_bucket(server: &TestServer, token: &str) -> TestResponse {
    let opts = CreateBucketOptions {
        ..Default::default()
    };

    server
        .post("/client/bucket")
        .json(&opts)
        .add_header(HEADER_NAME, token)
        .await
}

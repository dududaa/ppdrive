use axum_test::{TestResponse, TestServer};
use ppd_service::prelude::opts::{CreateUserClient, LoginUserClient};
use ppd_bk::models::bucket::CreateBucketOptions;

use crate::test_utils::HEADER_USER_KEY;

use super::HEADER_TOKEN_KEY;

#[allow(dead_code)]
pub async fn create_user_bucket(server: &TestServer, token: &str) -> TestResponse {
    let user_id = create_user_request(server, token).await.text();
    let opts = CreateBucketOptions::default();

    server
        .post("/client/user/bucket")
        .json(&opts)
        .add_header(HEADER_TOKEN_KEY, token)
        .add_header(HEADER_USER_KEY, user_id)
        .await
}

pub async fn create_user_request(server: &TestServer, token: &str) -> TestResponse {
    let data = CreateUserClient { max_bucket: None };
    server
        .post("/client/user/register")
        .json(&data)
        .add_header(HEADER_TOKEN_KEY, token)
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
        .add_header(HEADER_TOKEN_KEY, token)
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
        .add_header(HEADER_TOKEN_KEY, token)
        .await
}

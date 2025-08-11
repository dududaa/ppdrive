use axum_test::{TestResponse, TestServer};
use ppd_bk::{models::bucket::CreateBucketOptions, RBatis};

use crate::{
    opts::{CreateUserClient, LoginToken, LoginUserClient},
    ServerResult,
};
use super::{create_client_token, HEADER_NAME};

#[allow(dead_code)]
pub async fn create_user_bucket(server: &TestServer, token: &str) -> TestResponse {
    let opts = CreateBucketOptions::default();

    server
        .post("/client/user/bucket")
        .json(&opts)
        .authorization_bearer(token)
        .await
}

#[allow(dead_code)]
pub async fn get_user_token(server: &TestServer, db: &RBatis) -> ServerResult<String> {
    let token = create_client_token(&db).await?;
    let resp = login_user_request(&server, &token).await;
    let tokens: LoginToken = resp.json();

    Ok(tokens.access.0)
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

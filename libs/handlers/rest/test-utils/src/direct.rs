use std::sync::LazyLock;

use axum_test::{TestResponse, TestServer};
use ppd_shared::api::{UserCredentials, CreateBucketOptions};

static USER_CREDENTIALS: LazyLock<UserCredentials> = LazyLock::new(|| UserCredentials {
    username: "ppdriveuser".to_string(),
    password: "ppdriveUser@2025".to_string(),
});

#[allow(dead_code)]
pub async fn create_user_bucket(server: &TestServer, token: &str) -> TestResponse {
    let opts = CreateBucketOptions::default();

    server
        .post("/direct/user/bucket")
        .json(&opts)
        .authorization(token)
        .await
}

pub async fn register_user(server: &TestServer) -> TestResponse {
    server
        .post("/direct/user/register")
        .json(&*USER_CREDENTIALS)
        .await
}

pub async fn login_user_request(server: &TestServer) -> TestResponse {
    register_user(&server).await;
    
    server
        .post("/direct/user/login")
        .json(&*USER_CREDENTIALS)
        .await
}

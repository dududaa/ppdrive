use axum_test::multipart::{MultipartForm, Part};
use ppdrive_fs::{models::asset::AssetType, options::CreateAssetOptions};
use serial_test::serial;

use funtions::*;

use crate::{
    routes::tests::{app_config, create_db, create_server},
    AppResult,
};

#[tokio::test]
#[serial]
/// retrieve an authenticated user (created by client) using their access token
async fn test_client_user_get_userinfo() -> AppResult<()> {
    let config = app_config().await?;
    let db = create_db(&config).await?;

    let server = create_server(&config).await?;
    let token = get_user_token(&server, &db).await?;

    let resp = server.get("/client/user").authorization_bearer(&token);
    resp.await.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_user_create_bucket() -> AppResult<()> {
    let config = app_config().await?;
    let server = create_server(&config).await?;

    let db = create_db(&config).await?;
    let token = get_user_token(&server, &db).await?;

    let resp = create_user_bucket(&server, &token).await;
    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_user_create_asset() -> AppResult<()> {
    let config = app_config().await?;
    let server = create_server(&config).await?;

    let db = create_db(&config).await?;
    let token = get_user_token(&server, &db).await?;

    let bucket = create_user_bucket(&server, &token).await.text();
    let asset_path = "test-assets/great-folder";
    let mut asset_opts = CreateAssetOptions {
        asset_path: asset_path.to_string(),
        asset_type: AssetType::Folder,
        bucket,
        ..Default::default()
    };

    // this should fail without authorization
    let mut opts = serde_json::to_string(&asset_opts)?;
    let multipart = MultipartForm::new().add_text("options", &opts);
    let mut resp = server.post("/client/user/asset").multipart(multipart).await;

    resp.assert_status_not_ok();

    // create folder asset
    let multipart = MultipartForm::new().add_text("options", &opts);
    resp = server
        .post("/client/user/asset")
        .multipart(multipart)
        .authorization_bearer(&token)
        .await;

    resp.assert_status_ok();

    // upload file asset
    asset_opts.asset_path = format!("{asset_path}/test-file");
    asset_opts.asset_type = AssetType::File;
    opts = serde_json::to_string(&asset_opts)?;

    let file_bytes = include_bytes!("README.MD");
    let file_path = Part::bytes(file_bytes.as_slice())
        .file_name("some-test-file")
        .mime_type("text/markdown");

    let multipart = MultipartForm::new()
        .add_part("file", file_path)
        .add_text("options", &opts);

    resp = server
        .post("/client/user/asset")
        .multipart(multipart)
        .authorization_bearer(token)
        .await;

    resp.assert_status_ok();

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_user_delete_asset() -> AppResult<()> {
    let config = app_config().await?;
    let server = create_server(&config).await?;

    let db = create_db(&config).await?;
    let token = get_user_token(&server, &db).await?;

    let bucket = create_user_bucket(&server, &token).await.text();
    let asset_path = "delete-asset/great-folder";
    let asset_opts = CreateAssetOptions {
        asset_path: asset_path.to_string(),
        asset_type: AssetType::Folder,
        bucket,
        ..Default::default()
    };

    // create asset
    let opts = serde_json::to_string(&asset_opts)?;
    let multipart = MultipartForm::new().add_text("options", &opts);
    let _ = server
        .post("/client/user/asset")
        .multipart(multipart)
        .authorization_bearer(&token)
        .await;

    let path = format!("/client/user/asset/Folder/{asset_path}");
    let resp = server.delete(&path).authorization_bearer(&token).await;
    resp.assert_status_ok();

    Ok(())
}

mod funtions {
    use axum_test::{TestResponse, TestServer};
    use ppdrive_fs::{
        options::{CreateBucketOptions, LoginToken},
        RBatis,
    };

    use crate::{
        routes::tests::{client::login_user_request, create_client_token},
        AppResult,
    };

    pub async fn create_user_bucket(server: &TestServer, token: &str) -> TestResponse {
        let opts = CreateBucketOptions::default();

        server
            .post("/client/user/bucket")
            .json(&opts)
            .authorization_bearer(token)
            .await
    }

    pub async fn get_user_token(server: &TestServer, db: &RBatis) -> AppResult<String> {
        let token = create_client_token(&db).await?;
        let resp = login_user_request(&server, &token).await;
        let tokens: LoginToken = resp.json();

        Ok(tokens.access.0)
    }
}

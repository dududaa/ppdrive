use axum_test::multipart::{MultipartForm, Part};
use ppd_bk::models::asset::AssetType;
use serial_test::serial;

use ppd_fs::opts::CreateAssetOptions;

use rest_test_utils::{
    client::{create_client_bucket, create_user_bucket, create_user_request}, TestApp, HEADER_TOKEN_KEY, HEADER_USER_KEY
};
// mod test_utils;

#[tokio::test]
#[serial]
/// retrieve an authenticated user (created by client) using their access token
async fn test_client_user_get_userinfo() {
    let app = TestApp::new().await;
    let server = app.server();

    let token = app.client_token().await;
    let user_id = create_user_request(&server, &token).await.text();
    
    let resp = server
        .get("/client/user")
        .add_header(HEADER_TOKEN_KEY, token)
        .add_header(HEADER_USER_KEY, user_id);

    resp.await.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_client_user_create_bucket() {
    let app = TestApp::new().await;
    let server = app.server();

    let token = app.client_token().await;
    let resp = create_user_bucket(&server, &token).await;

    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_client_user_create_asset() {
    let app = TestApp::new().await;
    let server = app.server();

    let token = app.client_token().await;
    let bucket = create_client_bucket(&server, &token).await.text();

    let asset_path = "test-assets/great-folder";
    let mut asset_opts = CreateAssetOptions {
        asset_path: asset_path.to_string(),
        asset_type: AssetType::Folder,
        bucket,
        ..Default::default()
    };

    // this should fail without authorization
    let mut opts = asset_opts_str(&asset_opts);
    let multipart = MultipartForm::new().add_text("options", &opts);
    let mut resp = server.post("/client/user/asset").multipart(multipart).await;

    resp.assert_status_not_ok();

    // create folder asset
    let multipart = MultipartForm::new().add_text("options", &opts);
    let user_id = create_user_request(&server, &token).await.text();

    resp = server
        .post("/client/user/asset")
        .multipart(multipart)
        .add_header(HEADER_TOKEN_KEY, &token)
        .add_header(HEADER_USER_KEY, &user_id)
        .await;

    resp.assert_status_ok();

    // upload file asset
    asset_opts.asset_path = format!("{asset_path}/test-file");
    asset_opts.asset_type = AssetType::File;
    opts = asset_opts_str(&asset_opts);

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
        .add_header(HEADER_TOKEN_KEY, &token)
        .add_header(HEADER_USER_KEY, &user_id)
        .await;

    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_client_user_delete_asset() {
    let app = TestApp::new().await;
    let server = app.server();

    let token = app.client_token().await;
    let bucket = create_client_bucket(&server, &token).await.text();

    let asset_path = "delete-asset/great-folder";
    let asset_opts = CreateAssetOptions {
        asset_path: asset_path.to_string(),
        asset_type: AssetType::Folder,
        bucket,
        ..Default::default()
    };

    // create asset
    let opts = asset_opts_str(&asset_opts);
    let user_id = create_user_request(&server, &token).await.text();

    let multipart = MultipartForm::new().add_text("options", &opts);
    let _ = server
        .post("/client/user/asset")
        .multipart(multipart)
        .add_header(HEADER_TOKEN_KEY, &token)
        .add_header(HEADER_USER_KEY, &user_id)
        .await;

    let path = format!("/client/user/asset/Folder/{asset_path}");
    let resp = server
        .delete(&path)
        .add_header(HEADER_TOKEN_KEY, &token)
        .add_header(HEADER_USER_KEY, &user_id)
        .await;

    resp.assert_status_ok();
}

fn asset_opts_str(opts: &CreateAssetOptions) -> String {
    serde_json::to_string(opts).expect("unable to create strigify asset options")
}

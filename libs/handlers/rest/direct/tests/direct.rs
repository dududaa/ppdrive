use axum_test::multipart::{MultipartForm, Part};
use ppd_bk::models::asset::AssetType;
use serial_test::serial;

use ppd_fs::opts::CreateAssetOptions;

use rest_test_utils::{
    clean_up_test_assets, direct::{create_user_bucket, login_user_request, register_user}, TestApp
};

#[tokio::test]
#[serial]
async fn test_rest_direct_register_user() {
    let app = TestApp::new().await;
    let server = app.server();

    let resp = register_user(&server).await;
    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_rest_direct_login() {
    let app = TestApp::new().await;
    let server = app.server();

    let mut resp = register_user(&server).await;
    resp.assert_status_ok();

    resp = login_user_request(&server).await;
    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
/// retrieve an authenticated user using their access token
async fn test_direct_user_get_userinfo() {
    let app = TestApp::new().await;
    let server = app.server();

    let token = app.direct_login().await;
    let resp = server.get("/direct/user").authorization_bearer(&token);

    resp.await.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_direct_user_create_bucket() {
    let app = TestApp::new().await;
    let server = app.server();

    let token = app.direct_login().await;
    let resp = create_user_bucket(&server, &token).await;

    resp.assert_status_ok();
}

#[tokio::test]
#[serial]
async fn test_direct_user_create_asset() {
    clean_up_test_assets();

    let app = TestApp::new().await;
    let server = app.server();

    let token = app.direct_login().await;
    let bucket = create_user_bucket(&server, &token).await.text();

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
    let mut resp = server.post("/direct/user/asset").multipart(multipart).await;

    resp.assert_status_not_ok();

    // create folder asset
    let multipart = MultipartForm::new().add_text("options", &opts);
    resp = server
        .post("/direct/user/asset")
        .multipart(multipart)
        .authorization_bearer(&token)
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
        .post("/direct/user/asset")
        .multipart(multipart)
        .authorization_bearer(token)
        .await;

    resp.assert_status_ok();
    clean_up_test_assets();
}

#[tokio::test]
#[serial]
async fn test_direct_user_delete_asset() {
    clean_up_test_assets();

    let app = TestApp::new().await;
    let server = app.server();

    let token = app.direct_login().await;

    let bucket = create_user_bucket(&server, &token).await.text();
    let asset_path = "test-assets/great-folder";
    let asset_opts = CreateAssetOptions {
        asset_path: asset_path.to_string(),
        asset_type: AssetType::Folder,
        bucket,
        ..Default::default()
    };

    // create asset
    let opts = asset_opts_str(&asset_opts);
    let multipart = MultipartForm::new().add_text("options", &opts);
    let _ = server
        .post("/direct/user/asset")
        .multipart(multipart)
        .authorization_bearer(&token)
        .await;

    let path = format!("/direct/user/asset/Folder/{asset_path}");
    let resp = server.delete(&path).authorization_bearer(&token).await;
    
    resp.assert_status_ok();
    clean_up_test_assets();
}

fn asset_opts_str(opts: &CreateAssetOptions) -> String {
    serde_json::to_string(opts).expect("unable to create strigify asset options")
}

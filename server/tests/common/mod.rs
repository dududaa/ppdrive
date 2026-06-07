use axum_test::multipart::MultipartForm;
use axum_test::{TestRequest, TestServer};
use serde::Serialize;
use server::app::create_app;
use server::routers::payloads::{AssetType, UploadUrlConfig, UploadUrlMethod};

pub struct TestServerWrapper {
    server: TestServer,
}

impl TestServerWrapper {
    pub async fn new() -> anyhow::Result<TestServerWrapper> {
        let (app, _) = create_app().await?;
        let server = TestServer::new(app);

        let s = Self { server };
        Ok(s)
    }

    pub fn post<B: Serialize>(&self, url: &str, body: &B) -> TestRequest {
        self.server
            .post(url)
            .json(body)
            .content_type("application/json")
    }

    pub fn multipart(&self, url: &str, form: MultipartForm) -> TestRequest {
        self.server
            .post(url)
            .multipart(form)
    }
}

pub fn upload_config() -> UploadUrlConfig {
    UploadUrlConfig {
        method: UploadUrlMethod::Post,
        asset_type: AssetType::File,
        path: "test-assets/uploads/creator.jpg".to_string(),
        expires: 120,
        ..Default::default()
    }
}

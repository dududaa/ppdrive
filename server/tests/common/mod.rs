use axum::body::Bytes;
use axum_test::{TestRequest, TestServer, TestServerConfig, Transport};
use serde::Serialize;
use server::app::create_app;
use shared::server::{AssetType, UploadUrlConfig, UploadUrlMethod};

pub struct TestServerWrapper {
    server: TestServer,
}

impl TestServerWrapper {
    pub async fn new() -> anyhow::Result<TestServerWrapper> {
        let (app, _) = create_app().await?;
        let config = TestServerConfig {
            transport: Some(Transport::HttpRandomPort), // Enforces real networking
            ..Default::default()
        };

        let server = TestServer::new_with_config(app, config);

        let s = Self { server };
        Ok(s)
    }

    pub fn post<B: Serialize>(&self, url: &str, body: &B) -> TestRequest {
        self.server
            .post(url)
            .json(body)
            .content_type("application/json")
    }

    pub fn post_bytes(&self, url: &str, body: Bytes) -> TestRequest {
        self.server.post(url).bytes(body)
    }

    pub fn patch_bytes(&self, url: &str, body: Bytes) -> TestRequest {
        self.server.patch(url).bytes(body)
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

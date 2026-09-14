use axum::body::Bytes;
use axum_test::{TestRequest, TestServer, TestServerConfig, Transport};
use serde::Serialize;
use ppdrive_server::app::create_test_app;
use ppdrive::db::client::create_client;
use ppdrive::state::AppState;
use ppdrive::db::Database;

/// Clean all data from the test database to prevent test pollution.
pub async fn clean_db(db: &Database) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM file_permissions").execute(&**db).await?;
    sqlx::query("DELETE FROM assets").execute(&**db).await?;
    sqlx::query("DELETE FROM buckets").execute(&**db).await?;
    sqlx::query("DELETE FROM clients").execute(&**db).await?;
    sqlx::query("DELETE FROM users").execute(&**db).await?;
    sqlx::query("DELETE FROM asset_owner").execute(&**db).await?;
    Ok(())
}

pub struct TestServerWrapper {
    server: TestServer,
}

impl TestServerWrapper {
    pub async fn new() -> anyhow::Result<TestServerWrapper> {
        let (app, _) = create_test_app().await?;
        let config = TestServerConfig {
            transport: Some(Transport::HttpRandomPort),
            ..Default::default()
        };

        let server = TestServer::new_with_config(app, config);
        Ok(Self { server })
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

    pub fn get(&self, url: &str) -> TestRequest {
        self.server.get(url)
    }

    pub fn delete<B: Serialize>(&self, url: &str, body: &B) -> TestRequest {
        self.server
            .delete(url)
            .json(body)
            .content_type("application/json")
    }
}

/// Create a test client and return (state, client_token, client_header_key).
/// Cleans the database first to prevent test pollution from prior runs.
pub async fn setup_test_client() -> anyhow::Result<(AppState, String, String)> {
    let state = AppState::new().await?;
    clean_db(state.db()).await?;
    let client_header_key = state.config().client_header_key.clone();
    let client = create_client(state.db(), state.secrets(), "E2E Test Client").await?;
    Ok((state, client.token().to_string(), client_header_key))
}

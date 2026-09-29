// Shared across test binaries; individual binaries use different subsets.
#![allow(dead_code)]

use axum::body::Bytes;
use axum_test::{TestRequest, TestServer, TestServerConfig, Transport};
use ppdrive::db::Database;
use ppdrive::db::client::create_client;
use ppdrive::state::AppState;
use ppdrive_server::app::create_test_app;
use serde::Serialize;

/// Clean all data from the test database to prevent test pollution.
pub async fn clean_db(db: &Database) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM file_permissions")
        .execute(&**db)
        .await?;
    sqlx::query("DELETE FROM assets").execute(&**db).await?;
    sqlx::query("DELETE FROM buckets").execute(&**db).await?;
    sqlx::query("DELETE FROM clients").execute(&**db).await?;
    sqlx::query("DELETE FROM users").execute(&**db).await?;
    sqlx::query("DELETE FROM asset_owner")
        .execute(&**db)
        .await?;
    Ok(())
}

pub struct TestServerWrapper {
    server: TestServer,
    _dynamic_router: ppdrive_server::app::DynamicRouter,
}

impl TestServerWrapper {
    pub async fn new() -> anyhow::Result<TestServerWrapper> {
        let (app, _, _live_plugins) = create_test_app().await?;
        let config = TestServerConfig {
            transport: Some(Transport::HttpRandomPort),
            ..Default::default()
        };

        let server = TestServer::new_with_config(app, config);
        Ok(Self {
            server,
            _dynamic_router: _live_plugins,
        })
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

    pub fn get(&self, url: &str) -> TestRequest {
        self.server.get(url)
    }

    /// The bound port of the real HTTP transport, for tests that bypass
    /// the client and speak HTTP directly.
    pub fn port(&self) -> u16 {
        self.server
            .server_address()
            .and_then(|address| address.port_or_known_default())
            .expect("HTTP transport should expose a server port")
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

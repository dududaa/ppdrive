use axum::{Router, routing::IntoMakeService};
use axum_test::TestServer;
use handlers::prelude::opts::LoginToken;
use handlers::prelude::state::HandlerState;
use handlers::tools::create_client;
use ppd_bk::RBatis;
use ppd_shared::opts::ServiceConfig;
use ppd_shared::tools::AppSecrets;
use rest_client::load_router;

use crate::test_utils::functions::login_user_request;

pub mod functions;

const HEADER_NAME: &str = "x-ppd-client";

pub struct TestApp {
    pub db: RBatis,
    pub svc: IntoMakeService<Router<()>>,
}

impl TestApp {
    pub async fn new() -> Self {
        let ptr = load_router(10);
        let router = unsafe { Box::from_raw(ptr) };

        let config = ServiceConfig::default();

        let state = HandlerState::new(&config)
            .await
            .expect("unable to create app state");

        let db = state.db().clone();

        let svc = Router::new()
            .nest("/client", *router)
            .with_state(state)
            .into_make_service();

        Self { db, svc }
    }

    pub async fn client_token(&self) -> String {
        let secrets = AppSecrets::read()
            .await
            .expect("unable to create app secrets");

        let token = create_client(&self.db, &secrets, "Test Client")
            .await
            .expect("unable to create client token");

        token
    }

    #[allow(dead_code)]
    pub async fn user_token(&self) -> String {
        let token = self.client_token().await;
        let resp = login_user_request(&self.server(), &token).await;

        let tokens: LoginToken = resp.json();
        match tokens.access {
            Some(token) => token.0,
            None => panic!("unable to create user access token")
        }
    }

    pub fn server(&self) -> TestServer {
        TestServer::new(self.svc.clone()).expect("unable to create test server")
    }
}

use std::sync::Arc;

use axum::{Router, routing::IntoMakeService};
use axum_test::TestServer;
use ppd_bk::RBatis;
use ppd_bk::db::init_db;
use ppd_bk::db::migration::clean_db;
use ppd_service::prelude::opts::LoginToken;
use ppd_service::prelude::state::HandlerState;
use ppd_service::tools::create_client;
use ppd_shared::opts::ServiceConfig;
use ppd_shared::tools::{AppSecrets, root_dir};
use rest_client::load_router;

use crate::test_utils::functions::login_user_request;

pub mod functions;

pub const HEADER_TOKEN_KEY: &str = "ppd-client-token";
pub const HEADER_USER_KEY: &str = "ppd-client-user";

pub struct TestApp {
    pub db: RBatis,
    pub svc: IntoMakeService<Router<()>>,
    rt_ptr: *mut Router<HandlerState>,
}

impl TestApp {
    pub async fn new() -> Self {
        let rt_ptr = load_router(10);
        let router = (unsafe { &*rt_ptr }).clone();

        let db_filename = root_dir()
            .expect("cannot get root_dir")
            .join("test_db.sqlite");
        let db_filename = db_filename.to_str().expect("cannot extract db url");
        let db_url = format!("sqlite://{}", db_filename);

        if let Err(err) = clean_db().await {
            println!("{err}")
        }
        let db = init_db(&db_url).await.expect("unable to init database");
        let db = Arc::new(db);

        let mut config = ServiceConfig::default();
        config.base.db_url = db_url;

        let state = HandlerState::new(&config, db)
            .await
            .expect("unable to create app state");

        let db = state.db().clone();
        let svc = Router::new()
            .nest("/client", router)
            .with_state(state)
            .into_make_service();

        Self { db, svc, rt_ptr }
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
            None => panic!("unable to create user access token"),
        }
    }

    pub fn server(&self) -> TestServer {
        TestServer::new(self.svc.clone()).expect("unable to create test server")
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if !self.rt_ptr.is_null() {
            let _ = unsafe { Box::from_raw(self.rt_ptr) };
        }
    }
}

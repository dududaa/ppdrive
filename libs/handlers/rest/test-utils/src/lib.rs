use std::sync::Arc;

use axum::{Router, routing::IntoMakeService};
use axum_test::TestServer;
use ppd_bk::RBatis;
use ppd_bk::db::migration::clean_db;
use ppd_shared::opts::ServiceConfig;
pub use ppd_shared::start_logger;
use ppd_shared::{
    api::LoginTokens,
    tools::{AppSecrets, root_dir},
};
use ppdrive::prelude::state::HandlerState;
use ppdrive::tools::create_client;

use rest_client::rest_client as client_router;
use rest_direct::rest_direct as direct_router;

use crate::direct::login_user_request;

pub mod client;
pub mod direct;

pub struct TestApp {
    pub db: RBatis,
    pub svc: IntoMakeService<Router<()>>,
    client_rtr: *mut Router<HandlerState>,
    direct_rtr: *mut Router<HandlerState>,
}

impl TestApp {
    pub async fn new() -> Self {
        let db_filename = root_dir()
            .expect("cannot get root_dir")
            .join("test_db.sqlite");
        let db_filename = db_filename.to_str().expect("cannot extract db url");
        let db_url = format!("sqlite://{}", db_filename);

        if let Err(err) = clean_db().await {
            println!("{err}")
        }

        let mut config = ServiceConfig::default();
        config.base.db_url = db_url;

        let client_router = unsafe { client_router(Arc::into_raw(config.clone().into())) };
        let direct_router = unsafe { direct_router(Arc::into_raw(config.clone().into())) };

        let (client_rtr, client_router) = Self::unwrap_router(client_router);
        let (direct_rtr, direct_router) = Self::unwrap_router(direct_router);

        let state = HandlerState::new(&config)
            .await
            .expect("unable to create app state");

        let db = state.db().clone();
        let svc = Router::new()
            .nest("/client", client_router)
            .nest("/direct", direct_router)
            .with_state(state)
            .into_make_service();

        Self {
            db,
            svc,
            client_rtr,
            direct_rtr,
        }
    }

    pub async fn client_token(&self) -> String {
        let secrets = AppSecrets::read()
            .await
            .expect("unable to create app secrets");

        let client = create_client(&self.db, &secrets, "Test Client", None)
            .await
            .expect("unable to create client token");

        client.token().to_string()
    }

    #[allow(dead_code)]
    pub async fn direct_login(&self) -> String {
        let resp = login_user_request(&self.server()).await;

        let tokens: LoginTokens = resp.json();
        match tokens.access {
            Some(token) => format!("Bearer {}", token.0),
            None => panic!("unable to create user access token"),
        }
    }

    pub fn server(&self) -> TestServer {
        TestServer::new(self.svc.clone()).expect("unable to create test server")
    }

    fn unwrap_router(
        ptr: *mut Router<HandlerState>,
    ) -> (*mut Router<HandlerState>, Router<HandlerState>) {
        (ptr, unsafe { &*ptr }.clone())
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        unsafe {
            if !self.client_rtr.is_null() {
                let _ = Box::from_raw(self.client_rtr);
            }

            if !self.direct_rtr.is_null() {
                let _ = Box::from_raw(self.direct_rtr);
            }
        }
    }
}

pub fn clean_up_test_assets() {
    if let Err(err) = std::fs::remove_dir_all("test-assets") {
        println!("{err}");
    }
}

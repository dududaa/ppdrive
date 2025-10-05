use std::sync::Arc;

use crate::app::{serve_app, start_logger};
use errors::ServerError;
use ppd_service::prelude::state::HandlerState;
use ppd_bk::RBatis;
use ppd_shared::{opts::ServiceConfig, tools::init_secrets};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

#[no_mangle]
pub fn start_svc(config: Arc<ServiceConfig>, db: Arc<RBatis>, token: CancellationToken) {
    if let Ok(rt) = Runtime::new() {
        rt.block_on(async {
            let _guard = start_logger().expect("unable to start logger");
            init_secrets().await.expect("unable to initialize secrets");

            let state = HandlerState::new(&config, db)
                .await
                .expect("unable to create app state");

            serve_app(config, state, token)
                .await
                .expect("unable to serve app");
        })
    }
}

use std::sync::Arc;

use crate::app::serve_app;
use async_ffi::FutureExt;
use errors::ServerError;
use ppd_shared::{opts::internal::ServiceConfig, start_logger, tools::init_secrets};
use ppdrive::{
    errors::HandlerError, plugin::service::ServiceReturnType, prelude::state::HandlerState,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

#[no_mangle]
pub fn ppd_rest(config: Arc<ServiceConfig>, token: CancellationToken) -> ServiceReturnType {
    async move {
        Runtime::new()
            .map(|rt| {
                rt.block_on(async move {
                    let _guard = start_logger("ppd_rest=debug,tower_http=debug")
                        .expect("unable to start logger");

                    if let Err(err) = init_secrets().await {
                        tracing::error!("unable to initialize secrets: {err}");
                    }

                    match HandlerState::new(&config).await {
                        Ok(state) => {
                            if let Err(err) = serve_app(config, state, token).await {
                                tracing::error!("unable to serve app: {err}")
                            }
                        }
                        Err(err) => tracing::error!("unable to create app state: {err}"),
                    }
                })
            })
            .map_err(|err| HandlerError::InternalError(err.to_string()))
    }
    .into_ffi()
}

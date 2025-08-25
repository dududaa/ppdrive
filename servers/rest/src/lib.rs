use std::sync::Arc;

use crate::app::initialize_app;
use errors::ServerError;
use ppd_bk::db::migration::run_migrations;
use ppd_shared::plugins::service::{ServiceConfig, SharedConfig};
use tokio::runtime::Runtime;

// #[cfg(test)]
// mod tests;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

async fn run_server(config: SharedConfig) -> ServerResult<()> {
    
    run_migrations(&config.base.db_url).await?;
    let app = initialize_app(&config).await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &config.base.port)).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("listening on {addr}");
            }

            axum::serve(listener, app)
                .await
                .map_err(|err| ServerError::InitError(err.to_string()))?;
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
            panic!("{err}")
        }
    }

    Ok(())
}

#[no_mangle]
// #[repr(C)]
pub extern "C" fn start_server(config: *const ServiceConfig) {
    match Runtime::new() {
        Ok(rt) => {
            rt.block_on(async {
                let config = unsafe { Arc::from_raw(config) };
                if let Err(err) = run_server(config).await {
                    panic!("{err}")
                }
            });
        }
        Err(err) => panic!("{err}"),
    }
}

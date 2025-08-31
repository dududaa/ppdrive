use std::sync::Arc;

use crate::app::{initialize_app, start_logger};
use errors::ServerError;
use ppd_bk::db::migration::run_migrations;
use ppd_shared::plugins::service::ServiceConfig;
use tokio::runtime::Runtime;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

async fn launch_svc(config: Arc<ServiceConfig>) -> ServerResult<()> {
    let _guard = start_logger()?;
    run_migrations(&config.base.db_url).await?;

    let app = initialize_app(&config).await?;
    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &config.base.port)).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("new service listening on {addr}");
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
pub extern "C" fn start_svc(cfg_raw: *const ServiceConfig) {
    let config = unsafe { Arc::from_raw(cfg_raw) };
    
    match Runtime::new() {
        Ok(rt) => {
            rt.block_on(async {
                if let Err(err) = launch_svc(config).await {
                    tracing::error!("{err}");
                    panic!("{err}")
                }
            });
        }
        Err(err) => panic!("{err}"),
    }
}

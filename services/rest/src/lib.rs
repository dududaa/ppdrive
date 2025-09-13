use std::sync::Arc;

use crate::app::{initialize_app, start_logger};
use errors::ServerError;
use handlers::plugin::service::ServiceInfo;
use ppd_bk::db::migration::run_migrations;
use ppd_shared::opts::ServiceConfig;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

async fn launch_svc(config: Arc<ServiceConfig>) -> ServerResult<()> {
    let _guard = start_logger()?;

    run_migrations(&config.base.db_url).await?;
    initialize_app(&config).await?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn start_svc(config: *const ServiceConfig, svc: *const ServiceInfo) {
    let config = unsafe { Arc::from_raw(config) };
    let svc = unsafe { Arc::from_raw(svc) };

    svc.runtime.block_on(async {
        if let Err(err) = launch_svc(config).await {
            tracing::error!("{err}");
            panic!("{err}")
        }
    });
}

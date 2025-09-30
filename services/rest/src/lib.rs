use std::sync::Arc;

use crate::app::{initialize_app, start_logger};
use errors::ServerError;
use ppd_bk::RBatis;
use ppd_shared::opts::ServiceConfig;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

async fn launch_svc(
    config: Arc<ServiceConfig>,
    db: *const RBatis,
    token: *mut CancellationToken,
) -> ServerResult<()> {
    let _guard = start_logger()?;
    initialize_app(&config, db, token).await?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn start_svc(
    config: *const ServiceConfig,
    db: *const RBatis,
    token: *mut CancellationToken,
) {
    let config = unsafe { Arc::from_raw(config) };

    if let Ok(rt) = Runtime::new() {
        rt.block_on(async {
            if let Err(err) = launch_svc(config, db, token).await {
                tracing::error!("{err}");
                panic!("{err}")
            }
        });
    }
}

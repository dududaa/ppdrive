use std::sync::{mpsc::Sender, Arc};

use crate::app::{initialize_app, start_logger};
use errors::ServerError;
use ppd_bk::db::migration::run_migrations;
use ppd_shared::opts::ServiceConfig;
use tokio::runtime::Runtime;

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
pub extern "C" fn start_svc(config: *const ServiceConfig, tx: *const Sender<Arc<Runtime>>) {
    let config = unsafe { Arc::from_raw(config) };
    let tx = unsafe { Arc::from_raw(tx) };

    if let Ok(rt) = Runtime::new() {
        let rtc = Arc::new(rt);
        if let Err(err) = tx.send(rtc.clone()) {
            tracing::error!("unable to send service runtime: {err}")
        }

        rtc.block_on(async {
            if let Err(err) = launch_svc(config).await {
                tracing::error!("{err}");
                panic!("{err}")
            }
        });
    }
}

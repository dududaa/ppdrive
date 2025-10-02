use std::{sync::Arc};

use crate::app::{run_app, start_logger};
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
    db: Arc<RBatis>,
    token: CancellationToken,
) -> ServerResult<()> {
    let _guard = start_logger()?;
    run_app(config, db, token).await?;

    Ok(())
}

#[no_mangle]
pub fn start_svc(
    config: Arc<ServiceConfig>,
    db: Arc<RBatis>,
    token: CancellationToken,
) {
    if let Ok(rt) = Runtime::new() {
        rt.block_on(async {
            if let Err(err) = launch_svc(config, db, token).await {
                panic!("{err}")
            }
        })
    }
}

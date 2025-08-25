use crate::app::initialize_app;
use bincode::config;
use errors::ServerError;
use ppd_bk::db::migration::run_migrations;
use ppd_shared::plugins::service::ServiceConfig;
use tokio::runtime::Runtime;

// #[cfg(test)]
// mod tests;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

async fn run_server(config_data: &[u8]) -> ServerResult<()> {
    let (config, _): (ServiceConfig, usize) =
        bincode::decode_from_slice(config_data, config::standard())
            .map_err(|err| ServerError::InternalError(err.to_string()))?;
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
pub extern "C" fn start_server(config_data: *const u8, config_size: usize) {
    match Runtime::new() {
        Ok(rt) => {
            rt.block_on(async {
                let config = unsafe { std::slice::from_raw_parts(config_data, config_size) };
                if let Err(err) = run_server(config).await {
                    panic!("{err}")
                }
            });
        }
        Err(err) => panic!("{err}"),
    }
}

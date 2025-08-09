use crate::app::initialize_app;
use errors::ServerError;
use ppd_shared::config::AppConfig;
// use ppdrive_fs::config::{get_config_path, AppConfig};

mod app;
pub mod errors;
mod extractors;
mod general;

#[cfg(feature = "client")]
mod client;
mod state;
mod jwt;
mod opts;

type ServerResult<T> = Result<T, ServerError>;

pub async fn start_server() -> ServerResult<()> {
    let config = AppConfig::load().await?;
    let app = initialize_app(&config).await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.server().port())).await {
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
            panic!("{err:?}")
        }
    }

    Ok(())
}

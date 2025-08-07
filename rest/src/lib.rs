use crate::app::initialize_app;
use errors::RestError;
use ppdrive_fs::config::{get_config_path, AppConfig};

mod app;
pub mod errors;
mod routes;
mod state;
mod utils;

type AppResult<T> = Result<T, RestError>;

// #[tokio::main]
pub async fn start_server() -> AppResult<()> {
    let config_path = get_config_path()?;
    let config = AppConfig::load(config_path).await?;
    let app = initialize_app(&config).await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.server().port())).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("listening on {addr}");
            }

            axum::serve(listener, app)
                .await
                .map_err(|err| RestError::InitError(err.to_string()))?;
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
            panic!("{err:?}")
        }
    }

    Ok(())
}

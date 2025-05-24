use crate::app::create_app;
use errors::AppError;
use ppdrive_core::config::{AppConfig, ConfigUpdater};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{get_env, init_secrets};

mod app;
mod errors;
mod routes;
mod state;
mod utils;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppdrive=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut config = AppConfig::load().await?;

    if let Ok(url) = get_env("PPDRIVE_DATABASE_URL") {
        let mut data = ConfigUpdater::default();
        data.db_url = Some(url);
        config.update(data).await?;
    }

    // start ppdrive app
    init_secrets().await?;
    let app = create_app(&config).await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.server().port())).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("listening on {addr}");
            }

            axum::serve(listener, app)
                .await
                .map_err(|err| AppError::InitError(err.to_string()))?;
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
            panic!("{err:?}")
        }
    }

    Ok(())
}

#[cfg(test)]
pub mod main_test {
    use ppdrive_core::config::AppConfig;

    use crate::{errors::AppError, state::AppState};

    /// load .env creates and app state
    pub async fn pretest() -> Result<AppState, AppError> {
        let config = AppConfig::load().await?;
        AppState::new(&config).await
    }
}

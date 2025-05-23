use crate::app::create_app;
use config::{configor::update_db_url, AppConfig};
use errors::AppError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{args_runner, get_env, tools::secrets::generate_secrets_init};

mod app;
mod config;
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

    let mut config = AppConfig::build().await?;

    if let Ok(url) = get_env("PPDRIVE_DATABASE_URL") {
        update_db_url(&mut config, url).await?;
    }

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some() {
        return args_runner(args, &config).await;
    }

    // start ppdrive app
    generate_secrets_init().await?;
    let app = create_app(&config).await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.base().port())).await {
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
    use crate::{config::AppConfig, errors::AppError, state::AppState};

    /// load .env creates and app state
    pub async fn pretest() -> Result<AppState, AppError> {
        let config = AppConfig::build().await?;
        AppState::new(&config).await
    }
}

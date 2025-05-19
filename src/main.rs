use std::env::set_var;

use crate::app::create_app;
use dotenv::dotenv;
use errors::AppError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    get_env, run_args,
    tools::secrets::{generate_secrets_init, BEARER_KEY, BEARER_VALUE},
};

mod app;
mod config;
mod errors;
mod models;
mod routes;
mod state;
mod utils;

const DEFAULT_PORT: &str = "5000";

#[tokio::main]
async fn main() -> Result<(), AppError> {
    set_var(BEARER_KEY, BEARER_VALUE);
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppdrive=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some() {
        return run_args(args).await;
    }

    // start ppdrive app
    generate_secrets_init().await?;
    let port = get_env("PPDRIVE_PORT")
        .ok()
        .unwrap_or(DEFAULT_PORT.to_string());

    let app = create_app().await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
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
    use crate::{errors::AppError, state::AppState};

    /// load .env creates and app state
    pub async fn pretest() -> Result<AppState, AppError> {
        dotenv::dotenv().ok();
        AppState::new().await
    }
}

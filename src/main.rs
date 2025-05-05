use crate::app::create_app;
use dotenv::dotenv;
use errors::AppError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{client_keygen, get_env, ClientAccessKeys};

mod app;
mod errors;
mod models;
mod routes;
mod state;
mod utils;

const DEFAULT_PORT: &str = "5000";

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppdrive=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();

    // if specified, run ppdrive extra tools
    if let Some(a1) = args.get(1) {
        if a1 == "keygen" {
            let ClientAccessKeys {
                client_id,
                public,
                private,
            } = client_keygen().await?;
            tracing::info!(
                "
                Token generated successfully!

                PPD_PUBLIC: {public}
                PPD_PRIVATE: {private}
                CLIENT_ID: {client_id}
            "
            );
        }

        return Ok(());
    }

    // create tmp dir for managing uploaded assets
    // if let Err(err) = tokio::fs::create_dir("tmp").await {
    //     tracing::error!("unable to create tmp dir: {err}");
    // }

    // start ppdrive app
    let port = get_env("PPDRIVE_PORT")
        .ok()
        .unwrap_or(DEFAULT_PORT.to_string());

    let router = create_app().await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("listening on {addr}");
            }

            axum::serve(listener, router)
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

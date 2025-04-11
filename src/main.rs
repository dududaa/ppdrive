use dotenv::dotenv;
use errors::AppError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{get_env, client_keygen, ClientAccessKeys};
use crate::app::create_app;

mod app;
mod errors;
mod state;
mod utils;
mod models;
mod schema;
mod routes;

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
            let ClientAccessKeys{ client_id, public, private } = client_keygen().await?;
            tracing::info!("
                Token generated successfully!

                PPD_PUBLIC: {public}
                PPD_PRIVATE: {private}
                CLIENT_ID: {client_id}
            ");
        }

        return Ok(())
    }


    // create tmp dir for managing uploaded assets
    if let Err(err) = tokio::fs::create_dir("tmp").await {
        tracing::error!("unable to create tmp dir: {err}");
    }

    // start ppdrive app
    let port = get_env("PPDRIVE_PORT")?;
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

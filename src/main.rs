use dotenv::dotenv;
use errors::PPDriveError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{create_admin, get_env};
use crate::app::create_app;

mod app;
mod errors;
mod state;
mod utils;
mod models;
mod schema;
mod routes;

#[tokio::main]
async fn main() -> Result<(), PPDriveError> {
    dotenv().ok();

    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppdrive=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();

    if let Some(a1) = args.get(1) {
        if a1 == "create_admin" {
            let admin_id = create_admin().await?;
            tracing::info!("admin created successfully!");
            tracing::info!("\nadmin_id: {admin_id}");
        }

        return Ok(())
    }

    let port = get_env("PPDRIVE_PORT")?;
    let router = create_app().await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("listening on {addr}");
            }
    
            axum::serve(listener, router)
                .await
                .map_err(|err| PPDriveError::InitError(err.to_string()))?;
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
            panic!("{err:?}")
        }
    }

    Ok(())
}

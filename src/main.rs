use dotenv::dotenv;
use errors::PPDriveError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::get_env;
use crate::app::create_app;

mod app;
mod errors;
mod state;
mod utils;

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
                panic!("{}", &err.to_string())
            }
        }
    
        Ok(())
}

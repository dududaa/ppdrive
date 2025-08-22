use crate::app::initialize_app;
use errors::ServerError;
use ppd_bk::db::migration::run_migrations;
use ppd_shared::config::AppConfig;
use tokio::runtime::Runtime;

// #[cfg(test)]
// mod tests;

mod app;
mod errors;
pub type ServerResult<T> = Result<T, ServerError>;

async fn run_server(port: u16) -> ServerResult<()> {
    let config = AppConfig::load().await?;
    run_migrations(config.db().url()).await?;
    
    let app = initialize_app(&config).await?;

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
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

#[no_mangle]
pub extern "C" fn start_server(port: u16) {
    match Runtime::new() {
        Ok(rt) => {
            rt.block_on(async {
                if let Err(err) = run_server(port).await {
                    panic!("{err}")
                }
            });
        },
        Err(err) => panic!("{err}")
    }
}
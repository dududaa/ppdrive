use server::app::create_app;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (app, port) = create_app().await?;
    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &port)).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("new service listening on {addr}");
            }

            if let Err(err) = axum::serve(listener, app).await {
                tracing::error!("Error starting server: {err}");
            }
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
        }
    }

    Ok(())
}

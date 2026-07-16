use server::app::create_app;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    
    let (app, port) = create_app().await?;
    let port = args.first().map(|p| p.parse().ok().unwrap_or_default()).unwrap_or(port);
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

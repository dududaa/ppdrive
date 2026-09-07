use server::app::create_app;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    
    let (app, config_port) = create_app().await?;

    // Try to parse port from CLI args: accept "--port <N>" or "<N>" as first user arg
    let port = args.iter().position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .or_else(|| args.get(1))
        .and_then(|p| p.parse::<i16>().ok())
        .unwrap_or(config_port);

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

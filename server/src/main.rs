use server::app::{create_app, install_metrics};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    install_metrics()?;

    let (app, config_port) = create_app().await?;

    // Spawn background temp file cleanup (every hour, remove files older than 2 hours)
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(err) = shared::cleanup_tmp_files(std::time::Duration::from_secs(7200)).await {
                tracing::error!("tmp cleanup failed: {err}");
            }
        }
    });

    // Try to parse port from CLI args: accept "--port <N>" or "<N>" as first user arg
    let port = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .or_else(|| args.get(1))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(config_port);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &port)).await?;
    if let Ok(addr) = listener.local_addr() {
        tracing::info!("new service listening on {addr}");
    }

    let shutdown = async {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => {}
                _ = sigterm.recv() => {}
            }
        }

        #[cfg(not(unix))]
        {
            let _ = ctrl_c.await;
        }

        tracing::info!("shutdown signal received, draining connections...");
    };

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(shutdown)
        .await?;

    Ok(())
}

use anyhow::Context;
use ppdrive_server::app::{create_app, install_metrics};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    install_metrics()?;

    let (app, config_port, _live_plugins) = create_app().await?;

    // Spawn background temp file cleanup (every hour, remove files older than 2 hours)
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(err) = ppdrive::cleanup_tmp_files(std::time::Duration::from_secs(7200)).await
            {
                tracing::error!("tmp cleanup failed: {err}");
            }
        }
    });

    // Parse bind options from CLI args: "--host <ADDR>" and either
    // "--port <N>" or "<N>" as the first user arg; fall back to config.
    let (host, port) = parse_bind_args(&args, config_port);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .with_context(|| format!("failed to bind {host}:{port}"))?;
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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;

    Ok(())
}

/// Parse the bind address and port from the server's command-line arguments.
///
/// Accepts `--host <ADDR>` (defaults to `127.0.0.1`, the loopback interface;
/// pass `0.0.0.0` to serve across the local network) and either
/// `--port <N>` or a bare positional port; the configured port is the
/// fallback when neither form is present or valid.
fn parse_bind_args(args: &[String], config_port: u16) -> (String, u16) {
    let host = args
        .iter()
        .position(|a| a == "--host")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .or_else(|| args.get(1))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(config_port);
    (host, port)
}

#[cfg(test)]
mod tests {
    use super::parse_bind_args;

    fn args(argv: impl IntoIterator<Item = &'static str>) -> Vec<String> {
        argv.into_iter().map(String::from).collect()
    }

    #[test]
    fn defaults_to_loopback_and_config_port() {
        let (host, port) = parse_bind_args(&args(["server"]), 8000);
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 8000);
    }

    #[test]
    fn parses_host_override() {
        let (host, port) = parse_bind_args(&args(["server", "--host", "0.0.0.0"]), 8000);
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 8000);
    }

    #[test]
    fn parses_ipv6_host() {
        let (host, _) = parse_bind_args(&args(["server", "--host", "::1"]), 8000);
        assert_eq!(host, "::1");
    }

    #[test]
    fn parses_flag_port() {
        let (host, port) = parse_bind_args(&args(["server", "--port", "3000"]), 8000);
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 3000);
    }

    #[test]
    fn parses_positional_port_alongside_host() {
        // `ppdrive serve --port 3000 --host 0.0.0.0` forwards as
        // ["server", "3000", "--host", "0.0.0.0"]
        let (host, port) = parse_bind_args(&args(["server", "3000", "--host", "0.0.0.0"]), 8000);
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 3000);
    }

    #[test]
    fn host_without_port_falls_back_to_config_port() {
        // The first user arg is "--host", which must not be mistaken for a port.
        let (host, port) = parse_bind_args(&args(["server", "--host", "0.0.0.0"]), 9000);
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 9000);
    }

    #[test]
    fn invalid_port_falls_back_to_config_port() {
        let (_, port) = parse_bind_args(&args(["server", "not-a-port"]), 8000);
        assert_eq!(port, 8000);
        let (_, port) = parse_bind_args(&args(["server", "--port", "99999"]), 8000);
        assert_eq!(port, 8000);
    }
}

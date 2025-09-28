use std::sync::Arc;

use ppd_shared::opts::ServiceConfig;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_util::sync::CancellationToken;

use crate::ops::process_request;

mod ops;

pub type AppResult<T> = anyhow::Result<T>;
pub type Manager = Arc<ServiceManager>;

const DEFAULT_PORT: u16 = 5025;

#[derive(Debug)]
pub struct ServiceManager {
    pub tasks: Mutex<Vec<ServiceTask>>,

    /// cancellation token used for stopping this manager
    pub token: CancellationToken,

    port: u16,
}

impl ServiceManager {
    fn new(port: Option<u16>) -> Self {
        let mut manager = Self::default();
        if let Some(port) = port {
            manager.port = port;
        }

        manager
    }

    /// start the service manager at the provided port. this is a tcp listener opened at the
    /// connected port.
    async fn start(self) -> AppResult<()> {
        let token = self.token.clone();
        tokio::select! {
           run = self.run() => {
                if let Err(err) = run {
                    tracing::error!("cannot start ppdrive {err}")
                }
           }
           _ = token.cancelled() => {}
        }

        Ok(())
    }

    async fn run(self) -> AppResult<()> {
        let addr = format!("0.0.0.0:{}", self.port);

        let listener = TcpListener::bind(&addr).await?;
        let manager = Arc::new(self);

        loop {
            let tasks = manager.clone();
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    tokio::spawn(async move {
                        if let Err(err) = process_request(&mut socket, tasks).await {
                            tracing::error!("unable to process request: {err}")
                        }
                    });
                }
                Err(err) => {
                    tracing::error!("{err}");
                    break Ok(());
                }
            }
        }
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(vec![]),
            token: CancellationToken::new(),
            port: DEFAULT_PORT,
        }
    }
}

#[derive(Debug)]
pub struct ServiceTask {
    pub id: u8,
    pub config: ServiceConfig,
    pub token: Option<CancellationToken>,
}

impl ServiceTask {
    pub fn new(config: &ServiceConfig) -> Self {
        Self {
            id: rand::random(),
            config: config.clone(),
            token: None,
        }
    }
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let args: Vec<String> = std::env::args().collect();

    let port = args.get(1).map(|p| p.parse().unwrap_or(DEFAULT_PORT));

    let manager = ServiceManager::new(port);
    manager.start().await?;

    Ok(())
}

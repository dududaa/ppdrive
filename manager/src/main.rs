use std::sync::Arc;

use ppd_shared::{opts::ServiceConfig, start_logger};
use rbatis::RBatis;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_util::sync::CancellationToken;

use crate::ops::process_request;

mod ops;

#[cfg(test)]
mod tests;

#[cfg(test)]
use tokio::{net::TcpStream, task::JoinHandle};

const DEFAULT_PORT: u16 = 5025;

type AppResult<T> = anyhow::Result<T>;
type SharedManager = Arc<ServiceManager>;

struct Manager {
    inner: SharedManager,
}

impl Manager {
    fn new(port: Option<u16>) -> Self {
        let inner = ServiceManager::new(port);
        Self {
            inner: Arc::new(inner),
        }
    }

    /// start the service manager at the provided port. this is a tcp listener opened at the
    /// connected port.
    async fn start(&self) -> AppResult<()> {
        let token = self.token();
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

    async fn run(&self) -> AppResult<()> {
        let addr = self.addr();
        let listener = TcpListener::bind(&addr).await?;

        loop {
            let manager = self.inner.clone();
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    tokio::spawn(async move {
                        if let Err(err) = process_request(&mut socket, manager).await {
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

    fn addr(&self) -> String {
        format!("0.0.0.0:{}", self.inner.port)
    }

    fn token(&self) -> CancellationToken {
        self.inner.token.clone()
    }

    #[cfg(test)]
    async fn close(self) {
        self.inner.close().await;
    }

    #[cfg(test)]
    async fn connect(&self) -> AppResult<TcpStream> {
        let stream = TcpStream::connect(self.addr()).await?;
        Ok(stream)
    }

    #[cfg(test)]
    /// start manager in the background and return the handle
    async fn start_background(&self) -> JoinHandle<AppResult<()>> {
        use std::time::Duration;

        use tokio::time::sleep;

        let s = self.clone();
        let handle = tokio::spawn(async move { s.start().await });

        // wait a few seconds for tcp listener to be ready
        sleep(Duration::from_secs(5)).await;
        handle
    }

    #[cfg(test)]
    fn shared(&self) -> SharedManager {
        self.inner.clone()
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Clone for Manager {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Debug)]
struct ServiceManager {
    tasks: Mutex<Vec<ServiceTask>>,

    /// cancellation token used for stopping this manager
    token: CancellationToken,

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

    async fn close(&self) {
        // cancel running tasks/services
        let tasks = self.tasks.lock().await;
        if !tasks.is_empty() {
            tasks.iter().for_each(|t| {
                if let Some(token) = &t.token {
                    token.cancel();
                }
            });
        }

        // cancel manager
        self.token.cancel();
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
    db: Arc<RBatis>
}

impl ServiceTask {
    pub fn new(config: &ServiceConfig) -> Self {
        Self {
            id: rand::random(),
            config: config.clone(),
            token: None,
            db: Arc::new(RBatis::new())
        }
    }
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let _guard = start_logger("manager=debug");
    
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map(|p| p.parse().unwrap_or(DEFAULT_PORT));

    let manager = Manager::new(port);
    manager.start().await?;

    Ok(())
}

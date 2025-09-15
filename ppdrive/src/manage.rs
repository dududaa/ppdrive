use std::sync::Arc;

use bincode::{Decode, Encode, config};

use ppd_shared::{
    opts::ServiceConfig,
    plugin::{HasDependecies, Plugin},
};
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use tracing_appender::non_blocking::WorkerGuard;

use crate::{
    errors::{AppResult, Error},
    ops::{Response, ServiceCommand, process_request},
};
use handlers::plugin::service::Service;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

pub type TaskList = Arc<Mutex<Vec<ServiceTask>>>;

#[derive(Debug)]
pub struct ServiceManager {
    tasks: TaskList,
}

impl ServiceManager {
    /// start the service manager at the provided port. this is a tcp listener opened at the
    /// connected port.
    #[instrument]
    pub async fn start(&self, port: Option<u16>, _guard: WorkerGuard) -> AppResult<()> {
        let addr = Self::addr(port);
        let listener = TcpListener::bind(&addr).await?;

        // let tasks = Arc::new(Mutex::new(vec![]));
        tracing::info!("service manager listening at {}", addr);

        loop {
            let tasks = self.tasks.clone();
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

    /// add a new service to the manager
    pub async fn add(config: ServiceConfig, port: Option<u16>) -> AppResult<()> {
        let svc = Service::from(&config);
        tracing::info!(
            "starting service {:?} with auth modes {:?}",
            svc.ty(),
            svc.modes()
        );

        svc.preload_deps()?;
        svc.preload()?;

        // message service manager to load service
        let resp = Self::send_command::<u8>(ServiceCommand::Add(config), port).await?;
        resp.log();

        Ok(())
    }

    /// cancel a service in the manager
    pub async fn cancel(id: u8, port: Option<u16>) -> AppResult<()> {
        let resp = Self::send_command::<()>(ServiceCommand::Cancel(id), port).await?;
        resp.log();

        Ok(())
    }

    pub async fn list(port: Option<u16>) -> AppResult<()> {
        let resp = Self::send_command::<Vec<ServiceInfo>>(ServiceCommand::List, port).await?;
        let list = resp.body();

        resp.log();
        if !list.is_empty() {
            for svc in list {
                let id = svc.id;
                let port = svc.port;
                println!("id\t | port");
                println!("{id}\t | {port}")
            }
        } else {
            println!("no service running");
        }

        Ok(())
    }

    /// send a command to manager's tcp connection
    async fn send_command<T: Encode + Decode<()>>(
        cmd: ServiceCommand,
        port: Option<u16>,
    ) -> AppResult<Response<T>> {
        match bincode::encode_to_vec(cmd, config::standard()) {
            Ok(data) => {
                let addr = Self::addr(port);
                let mut stream = TcpStream::connect(addr).await?;
                stream.write_all(&data).await?;

                stream.readable().await?;
                let mut buf = [0u8; 1024];

                stream.try_read(&mut buf)?;
                let resp = bincode::decode_from_slice(&buf, config::standard())?;

                Ok(resp.0)
            }
            Err(err) => Err(Error::InternalError(format!(
                "unable to encode service command: {err}"
            ))),
        }
    }

    fn addr(port: Option<u16>) -> String {
        let port = port.unwrap_or(5025);
        format!("0.0.0.0:{}", port)
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        ServiceManager {
            tasks: Arc::new(Mutex::new(vec![])),
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

/// serializable [ServiceInfo].
#[derive(Encode, Decode, Clone)]
pub struct ServiceInfo {
    id: u8,
    port: u16,
}

impl From<&ServiceTask> for ServiceInfo {
    fn from(value: &ServiceTask) -> Self {
        ServiceInfo {
            id: value.id,
            port: value.config.base.port,
        }
    }
}

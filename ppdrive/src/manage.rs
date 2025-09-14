use bincode::{Decode, Encode, config};

use ppd_shared::{
    opts::ServiceConfig,
    plugin::{HasDependecies, Plugin},
};
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use tracing_appender::non_blocking::WorkerGuard;

use crate::errors::{AppResult, Error};
use handlers::plugin::service::Service;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct ServiceManager {
    list: Vec<ServiceInfo>,
}

impl ServiceManager {
    /// start the service manager at the provided port. this is a tcp listener opened at the
    /// connected port.
    #[instrument]
    pub async fn start(&mut self, port: Option<u16>, _guard: WorkerGuard) -> AppResult<()> {
        let addr = Self::addr(port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("service manager listening at {}", addr);

        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    // we're expecting one connection at a time, so we don't need to start
                    // multiple threads
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = socket.read(&mut buf).await {
                        if n > 0 {
                            match bincode::decode_from_slice::<ServiceCommand, _>(
                                &buf,
                                config::standard(),
                            ) {
                                Ok((cmd, _)) => match cmd {
                                    ServiceCommand::Add(config) => {
                                        let info = ServiceInfo::new(&config);
                                        let token = info.token.clone();

                                        let id = info.id.clone();

                                        tokio::spawn(async move {
                                            let handle = tokio::spawn(async move {
                                                let svc = Service::from(&config);
                                                tokio::select! {
                                                    _ = token.cancelled() => {
                                                        tracing::info!("service has been stopped...");
                                                    },
                                                    start = svc.start(config.clone()) => {
                                                        if let Err(err) = start {
                                                            tracing::error!("unable to start service: {err}")
                                                        }
                                                    },
                                                }
                                            });

                                            handle.await.ok()
                                        });

                                        self.list.push(info);

                                        let resp = Response::success(id).message(format!("service added to manager with id {id}. run 'ppdrive list' to see running services."));
                                        resp.write(&mut socket).await?;
                                    }

                                    ServiceCommand::Cancel(id) => {
                                        let item = self
                                            .list
                                            .iter()
                                            .enumerate()
                                            .find(|(_, item)| item.id == id);

                                        let resp = match item {
                                            Some((idx, item)) => {
                                                item.token.cancel();
                                                self.list.remove(idx);

                                                Response::success(()).message(format!("service {id} removed from manager successfully."))
                                            }
                                            None => Response::error(()).message(format!("unable to cancel service with id {id}. it's propably not running.")),
                                        };

                                        resp.write(&mut socket).await?;
                                    }

                                    ServiceCommand::List => {
                                        let items: Vec<ServiceItem> =
                                            self.list.iter().map(|s| s.into()).collect();

                                        let resp = Response::success(items).message(format!(
                                            "list generated for {} service(s)",
                                            self.list.len()
                                        ));

                                        resp.write(&mut socket).await?;
                                    }
                                },
                                Err(err) => {
                                    tracing::error!("unable to decode server config: {err}")
                                }
                            }
                        }
                    }
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
        let resp = Self::send_command::<Vec<ServiceItem>>(ServiceCommand::List, port).await?;
        let list = &resp.body;

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

                tracing::debug!("request sent");
                stream.readable().await?;
                tracing::debug!("reading response...");

                let mut buf = [0u8; 1024];
                let n = stream.try_read(&mut buf)?;

                tracing::debug!("response received {n}");
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
        ServiceManager { list: vec![] }
    }
}

#[derive(Debug)]
pub struct ServiceInfo {
    id: u8,
    config: ServiceConfig,
    token: CancellationToken,
}

impl ServiceInfo {
    pub fn new(config: &ServiceConfig) -> Self {
        Self {
            id: rand::random(),
            config: config.clone(),
            token: CancellationToken::new(),
        }
    }
}

/// serializable [ServiceInfo].
#[derive(Encode, Decode, Clone)]
pub struct ServiceItem {
    id: u8,
    port: u16,
}

impl From<&ServiceInfo> for ServiceItem {
    fn from(value: &ServiceInfo) -> Self {
        ServiceItem {
            id: value.id,
            port: value.config.base.port,
        }
    }
}

#[derive(Encode, Decode)]
/// service management commands
enum ServiceCommand {
    /// add a new service with the provided config
    Add(ServiceConfig),

    /// cancel and remove a service with the given id
    Cancel(u8),

    /// list running services
    List,
}

#[derive(Encode, Decode)]
struct Response<T: Encode + Decode<()>> {
    ty: ResponseType,
    body: T,
    msg: Option<String>,
}

impl<T: Encode + Decode<()>> Response<T> {
    fn success(body: T) -> Response<T> {
        Response {
            ty: ResponseType::Success,
            body,
            msg: None,
        }
    }

    fn error(body: T) -> Response<T> {
        Response {
            ty: ResponseType::Error,
            body,
            msg: None,
        }
    }

    fn message(mut self, msg: String) -> Self {
        self.msg = Some(msg);
        self
    }

    fn log(&self) {
        use ResponseType::*;

        let msg = self.msg.clone().unwrap_or("no message".to_string());

        match self.ty {
            Success => tracing::info!("{msg}"),
            Error => tracing::error!("{msg}"),
        }
    }

    async fn write(&self, socket: &mut TcpStream) -> AppResult<()> {
        let data = bincode::encode_to_vec(&self, config::standard())?;
        socket.write_all(&data).await?;

        Ok(())
    }
}

#[derive(Encode, Decode)]
enum ResponseType {
    Success,
    Error,
}

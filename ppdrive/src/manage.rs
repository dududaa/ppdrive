use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bincode::{Decode, Encode, config};

use ppd_shared::plugins::{
    HasDependecies,
    Plugin,
    service::{Service, ServiceConfig},
};

use crate::errors::{AppResult, Error};

pub struct ServiceManager {
    list: Vec<ServiceInfo>,
    // port: u16,
}

impl ServiceManager {
    /// start the service manager at the provided port
    pub fn start(&mut self, port: Option<u16>) -> AppResult<()> {
        let addr = Self::addr(port);
        let listener = TcpListener::bind(&addr)?;
        tracing::info!("service manager listening at {}...", addr);

        loop {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    // we're expecting one connection at a time, so we don't need to start
                    // multiple threads
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = socket.read(&mut buf) {
                        if n > 0 {
                            match bincode::decode_from_slice::<ServiceCommand, _>(
                                &buf,
                                config::standard(),
                            ) {
                                Ok((cmd, _)) => match cmd {
                                    ServiceCommand::Add(config) => {
                                        let info = ServiceInfo::new(config);
                                        let running = info.running.clone();

                                        let port = info.config.base.port;
                                        let id = info.id.clone();

                                        let config = info.config.clone();

                                        tokio::spawn(async move {
                                            // start the service
                                            while running.load(Ordering::Relaxed) {
                                                let svc = Service::from(&config);

                                                match svc.start(config.clone()) {
                                                    Ok(_) => tracing::info!(
                                                        "service {id} successfully started at port {port}"
                                                    ),
                                                    Err(err) => {
                                                        tracing::error!(
                                                            "unable to start service: {err}"
                                                        );
                                                        break;
                                                    }
                                                }
                                            }
                                        });

                                        let id = info.id;
                                        self.list.push(info);

                                        let resp = Response::success(id).message(format!("service added to manager with id {id}. run 'ppdrive list' to see running services."));
                                        resp.write(&mut socket)?;
                                    }

                                    ServiceCommand::Cancel(id) => {
                                        let item = self
                                            .list
                                            .iter()
                                            .enumerate()
                                            .find(|item| item.1.id == id);

                                        let resp = match item {
                                            Some((idx, info)) => {
                                                info.running.store(false, Ordering::Relaxed);
                                                self.list.remove(idx);

                                                Response::success(()).message(format!("service {id} removed from manager successfully."))
                                            }
                                            None => Response::error(()).message(format!("unable to cancel service with id {id}. it's propably not running.")),
                                        };

                                        resp.write(&mut socket)?;
                                    }

                                    ServiceCommand::List => {
                                        tracing::info!("listing commmand received");
                                        let resp =
                                            Response::success(self.list.clone()).message(format!(
                                                "list generated for {} service(s)",
                                                self.list.len()
                                            ));

                                        resp.write(&mut socket)?;
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
    pub fn add(config: ServiceConfig, port: Option<u16>) -> AppResult<()> {
        // preload service plugin
        let svc = Service::from(&config);
        tracing::info!(
            "starting service {:?} with auth modes {:?}",
            svc.ty(),
            svc.modes()
        );

        svc.preload_deps()?;
        svc.preload()?;

        // message service manager to load service
        let resp = Self::send_command::<u8>(ServiceCommand::Add(config), port)?;
        resp.log();

        Ok(())
    }

    /// cancel a service in the manager
    pub fn cancel(id: u8, port: Option<u16>) -> AppResult<()> {
        let resp = Self::send_command::<String>(ServiceCommand::Cancel(id), port)?;
        resp.log();

        Ok(())
    }

    pub fn list(port: Option<u16>) -> AppResult<()> {
        let resp = Self::send_command::<Vec<ServiceInfo>>(ServiceCommand::List, port)?;
        let list = &resp.body;

        resp.log();
        if !list.is_empty() {
            for svc in list {
                let id = svc.id;
                let port = svc.config.base.port;
                println!("id\t port");
                println!("{id}\t {port}")
            }
        } else {
            println!("no service started");
        }

        Ok(())
    }

    /// send a command to manager's tcp connection
    fn send_command<T: Encode + Decode<()>>(
        cmd: ServiceCommand,
        port: Option<u16>,
    ) -> AppResult<Response<T>> {
        match bincode::encode_to_vec(cmd, config::standard()) {
            Ok(data) => {
                let addr = Self::addr(port);
                let mut stream = TcpStream::connect(addr)?;
                stream.write_all(&data)?;

                let mut buf = [0u8; 1024];
                stream.read(&mut buf)?;

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

#[derive(Encode, Decode, Clone)]
pub struct ServiceInfo {
    id: u8,
    config: ServiceConfig,
    running: Arc<AtomicBool>,
}

impl ServiceInfo {
    fn new(config: ServiceConfig) -> Self {
        Self {
            id: rand::random(),
            config,
            running: Arc::new(AtomicBool::new(true)),
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

    fn write(&self, socket: &mut TcpStream) -> AppResult<()> {
        let data = bincode::encode_to_vec(&self, config::standard())?;
        socket.write_all(&data)?;

        Ok(())
    }
}

#[derive(Encode, Decode)]
enum ResponseType {
    Success,
    Error,
}

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use bincode::{Decode, Encode, config};
use ppd_shared::plugins::service::{ServiceBuilder, ServiceConfig};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::errors::AppResult;

pub struct ServiceManager {
    list: Vec<ServiceInfo>,
    // port: u16,
}

impl ServiceManager {
    pub fn start(&mut self, port: Option<u16>) -> AppResult<()> {
        let addr = Self::addr(port);
        let listener = TcpListener::bind(&addr)?;
        tracing::info!("service manager listening at {}...", addr);

        loop {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    // we're expecting one connection at a time, so we don't need to read stream
                    // from a new thread
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = socket.read(&mut buf) {
                        if n > 0 {
                            match bincode::decode_from_slice::<ServiceCommand, _>(
                                &buf,
                                config::standard(),
                            ) {
                                Ok((cmd, _)) => match cmd {
                                    ServiceCommand::Add(config) => {
                                        let port = config.base.port;
                                        let ty = config.base.ty;
                                        let info = ServiceInfo::new(config.clone());

                                        let id = info.id.clone();
                                        let token = info.token.clone();
                                        self.list.push(info);

                                        tracing::info!("service {} added to manager", id);

                                        // start the service
                                        let rt = Runtime::new()?;
                                        rt.block_on(async move {
                                            let svc = ServiceBuilder::new(ty).port(port).build();
                                            tokio::select! {
                                                res = svc.start(config) => {
                                                    match res {
                                                        Ok(_) => tracing::info!("service {id} started successfully"),
                                                        Err(err) => {
                                                            tracing::error!("unable to start service: {err}");
                                                            token.cancel();
                                                        }
                                                    }
                                                }
                                                _ = token.cancelled() => {
                                                    tracing::info!("service closed successfully")
                                                }
                                            }

                                        });
                                    }
                                    ServiceCommand::Cancel(id) => {
                                        let item = self
                                            .list
                                            .iter()
                                            .enumerate()
                                            .find(|item| item.1.id == id);

                                        match item {
                                            Some((idx, info)) => {
                                                info.token.cancel();
                                                self.list.remove(idx);
                                                tracing::info!(
                                                    "service {id} removed from manager successfully."
                                                );
                                            }
                                            None => tracing::error!(
                                                "unable to cancel service with id {id}. it's propably not running."
                                            ),
                                        }
                                    }
                                    ServiceCommand::List => {
                                        if !self.list.is_empty() {
                                            for svc in &self.list {
                                                let id = svc.id;
                                                let port = svc.config.base.port;
                                                println!("id\t port");
                                                println!("{id}\t {port}")
                                            } 
                                        } else {
                                            println!("no service started");
                                        }
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

    /// add a new task to the manager
    pub fn add(config: ServiceConfig, port: Option<u16>) -> AppResult<()> {
        Self::send_command(ServiceCommand::Add(config), port)?;
        Ok(())
    }

    /// cancel a service in the manager
    pub fn cancel(id: u8, port: Option<u16>) -> AppResult<()> {
        Self::send_command(ServiceCommand::Cancel(id), port)?;
        Ok(())
    }

    pub fn list(port: Option<u16>) -> AppResult<()> {
        Self::send_command(ServiceCommand::List, port)?;
        Ok(())
    }

    /// send a command to manager's tcp connection
    fn send_command(cmd: ServiceCommand, port: Option<u16>) -> AppResult<()> {
        match bincode::encode_to_vec(cmd, config::standard()) {
            Ok(data) => {
                let addr = Self::addr(port);
                let mut stream = TcpStream::connect(addr)?;
                stream.write_all(&data)?;
            }
            Err(err) => tracing::error!("unable to encode service command: {err}"),
        }

        Ok(())
    }

    fn addr(port: Option<u16>) -> String {
        let port = port.unwrap_or(5025);
        format!("0.0.0.0:{}", port)
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        ServiceManager {
            list: vec![],
            // port: 5025,
        }
    }
}

pub struct ServiceInfo {
    id: u8,
    config: ServiceConfig,
    token: CancellationToken,
}

impl ServiceInfo {
    fn new(config: ServiceConfig) -> Self {
        Self {
            id: rand::random(),
            config,
            token: CancellationToken::new(),
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
    List
}


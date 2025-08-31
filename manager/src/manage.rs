use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bincode::{Decode, Encode, config};
use ppd_shared::plugins::{service::{Service, ServiceConfig}, HasDependecies};
#[cfg(debug_assertions)]
use ppd_shared::plugins::Plugin;

use crate::errors::AppResult;

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
                    // we're expecting one connection at a time, so we don't need to start a new thread
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

                                        // start the service
                                        while running.load(Ordering::Relaxed) {
                                            let svc = Service::from(&config);

                                            match svc.start(config.clone()) {
                                                Ok(_) => tracing::info!(
                                                    "service {id} successfully started at port {port}"
                                                ),
                                                Err(err) => tracing::error!(
                                                    "unable to start service: {err}"
                                                ),
                                            }
                                        }

                                        self.list.push(info);
                                    }
                                    ServiceCommand::Cancel(id) => {
                                        let item = self
                                            .list
                                            .iter()
                                            .enumerate()
                                            .find(|item| item.1.id == id);

                                        match item {
                                            Some((idx, info)) => {
                                                info.running.store(false, Ordering::Relaxed);
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

    /// add a new service to the manager
    pub fn add(config: ServiceConfig, port: Option<u16>) -> AppResult<()> {
        // preload service plugin
        let svc = Service::from(&config);
        tracing::info!("starting service {:?} with auth modes {:?}", svc.ty(), svc.modes());

        svc.preload_deps()?;
        svc.preload()?;
        
        // message service manager to load service
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
        ServiceManager { list: vec![] }
    }
}

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

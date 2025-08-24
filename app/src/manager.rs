use std::{slice, sync::Arc};

use ppd_shared::{config::AppConfig, plugins::service::ServiceType};
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::{errors::AppResult, state::SyncState};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
pub struct ServiceManager {
    list: Vec<ServiceInfo>,
}

impl ServiceManager {
    #[instrument]
    pub async fn start(&mut self, state: SyncState) -> AppResult<()> {
        let state = state.lock().await;
        let config = state.config();

        let addr = config.db().manager_addr();
        let listener = TcpListener::bind(addr.clone()).await?;
        tracing::info!("service listener created at {addr}...");

        loop {
            tracing::info!("awaiting connection...");
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    // we're expecting one connection at a time, so we don't need to read stream
                    // from a new thread
                    let mut buf = [0u8; 1024];
                    if let Ok(n) = socket.read(&mut buf).await {
                        if n > 0 {
                            let cmd = unsafe { Arc::from_raw(buf.as_ptr() as *const ServiceCommand) };
                            match cmd.as_ref() {
                                ServiceCommand::Add(info) => {
                                    self.list.push(info.clone());
                                    tracing::info!("service {} added to manager", info.ty);
                                }
                                ServiceCommand::Cancel(id) => {
                                    let item = self.list.iter().enumerate().find(|item| item.1.ty == *id);
                                    if let Some((idx, info)) = item {
                                        info.token.cancel();
                                        self.list.remove(idx);
                                        tracing::info!("service {} removed from manager", id);
                                    }
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
    pub async fn add_svc(svc_info: ServiceInfo, state: SyncState) -> AppResult<()> {
        let state = state.lock().await;
        let config = state.config();

        Self::send_command(ServiceCommand::Add(svc_info), config).await?;
        Ok(())
    }

    /// cancel a task in the manager
    pub async fn cancel_svc(ty: ServiceType, state: SyncState) -> AppResult<()> {
        let state = state.lock().await;
        let config = state.config();

        Self::send_command(ServiceCommand::Cancel(ty), config).await?;
        Ok(())
    }

    async fn send_command(cmd: ServiceCommand, config: &AppConfig) -> AppResult<()> {
        let svc = Arc::new(cmd);
        let svc = Arc::into_raw(svc);

        let svc_size = std::mem::size_of::<ServiceCommand>();
        let data = unsafe { slice::from_raw_parts(svc as *const u8, svc_size) };

        let addr = config.db().manager_addr();
        let mut stream = TcpStream::connect(addr).await?;
        stream.write_all(data).await?;

        Ok(())
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        ServiceManager { list: vec![] }
    }
}

#[derive(Clone, Debug)]
pub struct ServiceInfo {
    pub ty: ServiceType,
    pub token: CancellationToken,
}

enum ServiceCommand {
    Add(ServiceInfo),
    Cancel(ServiceType),
}

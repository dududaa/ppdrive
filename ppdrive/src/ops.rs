use std::sync::atomic::Ordering;

use bincode::{Decode, Encode, config};
use handlers::plugin::service::Service;
use ppd_shared::{opts::{ServiceConfig, ServiceRequest}, plugin::PluginTransport};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

use crate::{
    errors::{AppResult, Error},
    manage::{ServiceInfo, ServiceTask, Manager},
};

/// adds a new service to the task pool
async fn start_service(
    manager: Manager,
    config: ServiceConfig,
    socket: &mut TcpStream,
    server_addr: String,
) -> AppResult<()> {
    let mut task = ServiceTask::new(&config);
    
    let tx = PluginTransport::new(Some(server_addr));
    let tx_clone = tx.clone();
    
    tracing::debug!("spawning new service...");
    tokio::spawn(async move {
        let svc = Service::from(&config);
        tracing::debug!("calling start with tx");
        if let Err(err) = svc.start::<CancellationToken>(config.clone(), tx_clone) {
            tracing::error!("unable to start server: {err}")
        }
    });
    
    // wait for token to be sent by tx
    let token_await = manager.token_await.load(Ordering::Relaxed);
    
    loop {
        tracing::debug!("listening for input...");
        if !token_await {
            tracing::debug!("incoming token received...");
            manager.token_await.store(true, Ordering::Relaxed);
            break;
        }
    }

    match tx.recv().await {
        Some(token) => task.token = Some(token),
        None => tracing::error!("could not receive token"),
    }

    let mut tasks = manager.tasks.lock().await;
    let id = task.id.clone();
    tasks.push(task);

    std::mem::drop(tasks); // drop tasks MutexGuard to prevent deadlock
    let resp = Response::success(id)
        .message(format!(
            "service added to manager with id {id}. run 'ppdrive list' to see running services."
        ));

    resp.write(socket).await?;

    Ok(())
}

/// stop a running service with the given id
async fn stop_service(
    manager: Manager,
    id: u8,
    socket: &mut TcpStream,
) -> AppResult<()> {
    let mut tasks = manager.tasks.lock().await;
    let item = tasks.iter().enumerate().find(|(_, item)| item.id == id);

    let resp = match item {
        Some((idx, item)) => {
            if let Some(token) = &item.token {
                token.cancel();
            }

            tasks.remove(idx);
            Response::success(())
                .message(format!("service {id} removed from manager successfully."))
        }
        None => Response::error(()).message(format!(
            "unable to cancel service with id {id}. it's propably not running."
        )),
    };

    resp.write(socket).await?;

    Ok(())
}

/// list running services
async fn list_services(manager: Manager, socket: &mut TcpStream) -> AppResult<()> {
    let tasks = manager.tasks.lock().await;
    let items: Vec<ServiceInfo> = tasks.iter().map(|s| s.into()).collect();

    let resp =
        Response::success(items).message(format!("list generated for {} service(s)", tasks.len()));

    resp.write(socket).await?;

    Ok(())
}

pub async fn process_request(
    socket: &mut TcpStream,
    manager: Manager,
    server_addr: String,
) -> AppResult<()> {
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;

    if n <= 0 {
        return Err(Error::InternalError("invalid packet received".to_string()));
    }
    
    let (req, _) = bincode::decode_from_slice::<ServiceRequest, _>(&buf, config::standard())?;

    match req {
        ServiceRequest::Add(config) => {
            start_service(manager, config, socket, server_addr).await
        }

        ServiceRequest::Cancel(id) => {
            stop_service(manager, id, socket).await
        }

        ServiceRequest::List => {
            list_services(manager, socket).await
        }

        ServiceRequest::TokenReceived => {
            manager.token_await.store(false, Ordering::Relaxed);
            Ok(())
        }
    }
}

#[derive(Encode, Decode)]
pub struct Response<T: Encode + Decode<()>> {
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

    pub fn log(&self) {
        use ResponseType::*;

        let msg = self.msg.clone().unwrap_or("no message".to_string());

        match self.ty {
            Success => tracing::info!("{msg}"),
            Error => tracing::error!("{msg}"),
        }
    }

    pub fn body(&self) -> &T {
        &self.body
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
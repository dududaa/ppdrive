use std::sync::atomic::Ordering;

use bincode::{Decode, Encode, config};
use handlers::{
    db::{init_db, migration::run_migrations},
    plugin::service::Service, tools::create_client,
};
use ppd_shared::{opts::ServiceConfig, tools::AppSecrets};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

use crate::{
    errors::{AppResult, Error},
    manage::{Manager, ServiceInfo, ServiceTask},
};

/// adds a new service to the task pool
async fn start_service(
    manager: Manager,
    config: ServiceConfig,
    socket: &mut TcpStream,
) -> AppResult<()> {
    let token = CancellationToken::new();
    let mut task = ServiceTask::new(&config);
    task.token = Some(token.clone());

    tokio::spawn(async move {
        let svc = Service::from(&config);
        if let Err(err) = svc.start::<CancellationToken>(config.clone(), token) {
            tracing::error!("unable to start server: {err}")
        }
    });

    let mut tasks = manager.tasks.lock().await;
    let id = task.id.clone();
    tasks.push(task);

    std::mem::drop(tasks); // drop tasks MutexGuard to prevent deadlock
    let resp = Response::success(id).message(format!(
        "service added to manager with id {id}. run 'ppdrive list' to see running services."
    ));

    resp.write(socket).await?;

    Ok(())
}

/// stop a running service with the given id
async fn stop_service(manager: Manager, id: u8, socket: &mut TcpStream) -> AppResult<()> {
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

/// create new client for a specified
async fn create_new_client(
    manager: Manager,
    svc_id: u8,
    client_name: String,
) -> AppResult<String> {
    let tasks = manager.tasks.lock().await;
    let task = tasks.iter().find(|t| t.id == svc_id).ok_or(Error::InternalError(format!("service with id {svc_id} does not exist.")))?;
    
    let db_url = &task.config.base.db_url;
    run_migrations(db_url)
        .await
        .map_err(|err| Error::InternalError(format!("unable to run migration: {err}")))?;

    let db = init_db(db_url)
        .await
        .map_err(|err| Error::InternalError(format!("unable to get db instance: {err}")))?;

    let secrets = AppSecrets::read().await?;

    let token = create_client(&db, &secrets, &client_name).await?;

    Ok(token)
}

pub async fn process_request(socket: &mut TcpStream, manager: Manager) -> AppResult<()> {
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;

    if n <= 0 {
        return Err(Error::InternalError("invalid packet received".to_string()));
    }

    let (req, _) = bincode::decode_from_slice::<ServiceRequest, _>(&buf, config::standard())?;

    match req {
        ServiceRequest::Add(config) => start_service(manager, config, socket).await,

        ServiceRequest::Cancel(id) => stop_service(manager, id, socket).await,

        ServiceRequest::List => list_services(manager, socket).await,

        ServiceRequest::CreateClient(svc_id, client_name) => {
            let resp = match create_new_client(manager, svc_id, client_name).await {
                Ok(token) => Response::success(()).message(format!("client token {token}")),
                Err(err) => Response::error(()).message(err.to_string())
            };

            resp.write(socket).await?;
            Ok(())
        }

        ServiceRequest::TokenReceived => {
            manager.token_await.store(false, Ordering::SeqCst);
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

#[derive(Encode, Decode, Debug)]
/// service management request type
pub enum ServiceRequest {
    /// add a new service with the provided config
    Add(ServiceConfig),

    /// cancel and remove a service with the given id
    Cancel(u8),

    /// list running services
    List,

    CreateClient(u8, String),

    /// a request to confirm that service token has been sent to this management server
    TokenReceived,
}

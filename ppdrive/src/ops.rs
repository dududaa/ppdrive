use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bincode::{Decode, Encode, config};
use handlers::plugin::service::Service;
use ppd_shared::{opts::ServiceConfig, plugin::PluginTransport};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, task::{JoinError, JoinHandle}
};

use crate::{
    errors::{AppResult, Error},
    manage::{ServiceInfo, ServiceTask, TaskList},
}; 

/// adds a new service to the task pool
async fn start_service(
    tasks: TaskList,
    config: ServiceConfig,
    socket: &mut TcpStream,
) -> AppResult<()> {
    let mut tasks = tasks.lock().await;
    let mut task = ServiceTask::new(&config);
    let id = task.id.clone();

    let tx = PluginTransport::new();
    let tx_clone = tx.clone();

    tracing::debug!("spawning new servince...");
    tokio::spawn(async move {
        let svc = Service::from(&config);
        if let Err(err) = svc.start(config.clone(), tx_clone) {
            tracing::error!("unable to start server: {err}")
        }
    });

    tracing::debug!("awaiting token...");
    match tx.recv().await {
        Some(token) => {
            tracing::debug!("service token received");
            task.token = Some(token)
        }
        None => tracing::error!("could not receive token")
    }

    tasks.push(task);
    std::mem::drop(tasks); // drop tasks MutexGuard to prevent deadlock

    let resp = Response::success(id).message(format!(
        "service added to manager with id {id}. run 'ppdrive list' to see running services."
    ));
    resp.write(socket).await.ok();

    Ok(())
}

/// stop a running service with the given id
async fn stop_service(
    tasks: &mut Vec<ServiceTask>,
    id: u8,
    socket: &mut TcpStream,
) -> AppResult<()> {
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
async fn list_services(tasks: &mut Vec<ServiceTask>, socket: &mut TcpStream) -> AppResult<()> {
    let items: Vec<ServiceInfo> = tasks.iter().map(|s| s.into()).collect();

    let resp =
        Response::success(items).message(format!("list generated for {} service(s)", tasks.len()));

    resp.write(socket).await?;

    Ok(())
}

pub async fn process_request(socket: &mut TcpStream, tasks: TaskList) -> AppResult<()> {
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;

    if n <= 0 {
        return Err(Error::InternalError("invalid packet received".to_string()));
    }

    let (cmd, _) = bincode::decode_from_slice::<ServiceCommand, _>(&buf, config::standard())?;

    match cmd {
        ServiceCommand::Add(config) => start_service(tasks.clone(), config, socket).await,

        ServiceCommand::Cancel(id) => {
            let mut tasks = tasks.lock().await;
            stop_service(&mut tasks, id, socket).await
        }

        ServiceCommand::List => {
            let mut tasks = tasks.lock().await;
            list_services(&mut tasks, socket).await
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

#[derive(Encode, Decode)]
/// service management commands
pub enum ServiceCommand {
    /// add a new service with the provided config
    Add(ServiceConfig),

    /// cancel and remove a service with the given id
    Cancel(u8),

    /// list running services
    List,
}

#[derive(Debug)]
pub struct TaskHandle<T>(JoinHandle<T>);
impl<T> Future for TaskHandle<T> {
    type Output = Result<T, JoinError>;
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut self.0) }.poll(cx)
    }
}

impl<T> Drop for TaskHandle<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn spawn_service<T>(future: T) -> TaskHandle<T::Output>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    TaskHandle(tokio::spawn(future))
}

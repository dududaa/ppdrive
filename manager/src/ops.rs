use std::sync::Arc;

use anyhow::anyhow;
use bincode::config;
use handlers::{db::init_db, plugin::service::Service, tools::create_client};
use ppd_shared::{
    opts::{Response, ServiceConfig, ServiceInfo, ServiceRequest},
    tools::AppSecrets,
};
use tokio::{io::AsyncReadExt, net::TcpStream};
use tokio_util::sync::CancellationToken;

use crate::{AppResult, ServiceManager, ServiceTask, SharedManager};

/// adds a new service to the task pool
pub async fn start_service(
    manager: SharedManager,
    config: ServiceConfig,
    socket: &mut TcpStream,
) -> AppResult<u8> {
    let db_url = &config.base.db_url;
    let db = init_db(&db_url).await.map_err(|err| anyhow!(err))?;

    let token = CancellationToken::new();
    let db = Arc::new(db);

    let mut task = ServiceTask::new(&config);
    task.token = Some(token.clone());
    task.db = db.clone();

    let mut tasks = manager.tasks.lock().await;
    let id = task.id.clone();
    tasks.push(task);

    std::mem::drop(tasks); // drop tasks MutexGuard to prevent deadlock
    let resp = Response::success(id.clone()).message(format!(
        "service added to manager with id {id}. run 'ppdrive list' to see running services."
    ));

    resp.write(socket)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;

    tokio::spawn(async move {
        let svc = Service::from(&config);
        svc.start(config.clone(), db, token)
            .await
            .expect("unable to start service");
    });

    Ok(id)
}

/// stop a running service with the given id
pub async fn stop_service(manager: SharedManager, id: u8, socket: &mut TcpStream) -> AppResult<()> {
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

    resp.write(socket)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;

    Ok(())
}

/// list running services
pub async fn list_services(manager: SharedManager, socket: &mut TcpStream) -> AppResult<()> {
    let tasks = manager.tasks.lock().await;
    let items: Vec<ServiceInfo> = tasks.iter().map(|s| s.into()).collect();

    let resp =
        Response::success(items).message(format!("list generated for {} service(s)", tasks.len()));

    resp.write(socket)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;

    Ok(())
}

/// create new client for a specified
pub async fn create_new_client(
    manager: SharedManager,
    svc_id: u8,
    client_name: String,
) -> AppResult<String> {
    let tasks = manager.tasks.lock().await;
    let task = tasks
        .iter()
        .find(|t| t.id == svc_id)
        .ok_or(anyhow::Error::msg(format!(
            "service with id {svc_id} does not exist."
        )))?;

    let secrets = AppSecrets::read().await.map_err(|err| anyhow!(err))?;
    let token = create_client(&task.db, &secrets, &client_name)
        .await
        .map_err(|err| anyhow!(err))?;

    Ok(token)
}

pub async fn process_request(
    socket: &mut TcpStream,
    manager: Arc<ServiceManager>,
) -> AppResult<()> {
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;

    if n <= 0 {
        return Err(anyhow!("invalid packet received"));
    }

    let (req, _) = bincode::decode_from_slice::<ServiceRequest, _>(&buf, config::standard())?;

    match req {
        ServiceRequest::Add(config) => {
            start_service(manager, config, socket).await?;
            Ok(())
        }

        ServiceRequest::Cancel(id) => stop_service(manager, id, socket).await,

        ServiceRequest::List => list_services(manager, socket).await,

        ServiceRequest::Stop => {
            manager.close().await;
            let resp = Response::success(()).message("manager has been closed successully");
            resp.write(socket).await.expect("unable to write response");

            Ok(())
        }

        ServiceRequest::CreateClient(svc_id, client_name) => {
            let resp = match create_new_client(manager, svc_id, client_name).await {
                Ok(token) => Response::success(()).message(format!("client created: {token}")),
                Err(err) => Response::error(()).message(err.to_string()),
            };

            resp.write(socket).await.map_err(|err| anyhow!(err))?;
            Ok(())
        }
    }
}

impl From<&ServiceTask> for ServiceInfo {
    fn from(value: &ServiceTask) -> Self {
        Self {
            id: value.id,
            port: value.config.base.port,
        }
    }
}

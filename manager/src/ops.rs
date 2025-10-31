use std::sync::Arc;

use anyhow::anyhow;
use bincode::config;
use ppd_shared::{
    opts::{ClientDetails, ClientInfo, Response, ServiceConfig, ServiceInfo, ServiceRequest},
    tools::AppSecrets,
};
use ppdrive::{
    db::init_db,
    plugin::service::Service,
    tools::{create_client, get_clients, regenerate_token},
};
use tokio::{io::AsyncReadExt, net::TcpStream};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{AppResult, ServiceManager, ServiceTask, SharedManager};

/// adds a new service to the task pool
pub async fn start_service(
    manager: SharedManager,
    config: ServiceConfig,
    socket: &mut TcpStream,
) -> AppResult<u8> {
    let db_url = &config.base.db_url;
    let db = init_db(db_url).await.map_err(|err| anyhow!(err))?;

    let token = CancellationToken::new();
    let db = Arc::new(db);

    let mut task = ServiceTask::new(&config);
    task.token = Some(token.clone());
    task.db = db.clone();

    let mut tasks = manager.tasks.lock().await;
    let id = task.id;
    tasks.push(task);

    std::mem::drop(tasks); // drop tasks MutexGuard to prevent deadlock
    let resp = Response::success(id).message(format!("service added to manager with id {id}."));

    resp.write(socket)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;

    tokio::spawn(
        async move {
            let svc = Service::from(&config);
            if let Err(err) = svc.start(config.clone(), db, token).await {
                tracing::error!("service {id} failure: {err}")
            }
        }
        .instrument(tracing::info_span!("ppd_start_service")),
    );

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
async fn create_new_client(
    manager: SharedManager,
    svc_id: u8,
    client_name: String,
    max_bucket_size: Option<f64>,
) -> AppResult<ClientDetails> {
    let task = manager.get_task(svc_id).await?;
    let secrets = AppSecrets::read().await.map_err(|err| anyhow!(err))?;

    let client = create_client(&task.db, &secrets, &client_name, max_bucket_size)
        .await
        .map_err(|err| anyhow!(err))?;

    Ok(client)
}

/// create new client for a specified
async fn refresh_client_token(
    manager: SharedManager,
    svc_id: u8,
    client_id: String,
) -> AppResult<String> {
    let task = manager.get_task(svc_id).await?;
    let secrets = AppSecrets::read().await.map_err(|err| anyhow!(err))?;

    let token = regenerate_token(&task.db, &secrets, &client_id)
        .await
        .map_err(|err| anyhow!(err))?;

    Ok(token)
}

/// create new client for a specified
async fn get_client_list(manager: SharedManager, svc_id: u8) -> AppResult<Vec<ClientInfo>> {
    let task = manager.get_task(svc_id).await?;
    let clients = get_clients(&task.db).await.map_err(|err| anyhow!(err))?;

    Ok(clients)
}

pub async fn process_request(
    socket: &mut TcpStream,
    manager: Arc<ServiceManager>,
) -> AppResult<()> {
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;

    if n == 0 {
        return Ok(());
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

        ServiceRequest::CreateClient(svc_id, client_name, bucket_size) => {
            let resp = match create_new_client(manager, svc_id, client_name, bucket_size).await {
                Ok(client) => Response::success(Some(client))
                    .message("client created successfully."),
                Err(err) => Response::error(None).message(err.to_string()),
            };

            resp.write(socket).await.map_err(|err| anyhow!(err))?;
            Ok(())
        }

        ServiceRequest::RefreshClientToken(svc_id, client_id) => {
            let resp = match refresh_client_token(manager, svc_id, client_id).await {
                Ok(token) => Response::success(Some(token))
                    .message("client token regenerated successfully."),
                Err(err) => Response::error(None).message(err.to_string()),
            };

            resp.write(socket).await.map_err(|err| anyhow!(err))?;
            Ok(())
        }

        ServiceRequest::GetClientList(svc_id) => {
            let resp = match get_client_list(manager, svc_id).await {
                Ok(clients) => {
                    let len = clients.len();
                    Response::success(clients).message(format!("total {} clients available.", len))
                }
                Err(err) => Response::error(vec![]).message(err.to_string()),
            };

            resp.write(socket).await.map_err(|err| anyhow!(err))?;
            Ok(())
        }

        ServiceRequest::CheckStatus => {
            let resp = Response::success(());
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
            ty: value.config.ty,
            auth_modes: value.config.auth.modes.clone(),
        }
    }
}

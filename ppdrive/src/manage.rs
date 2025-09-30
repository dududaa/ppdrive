use bincode::{Decode, Encode, config};

use ppd_shared::opts::{Response, ServiceConfig, ServiceInfo, ServiceRequest};

use crate::errors::{AppResult, Error};
use handlers::plugin::service::Service;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[derive(Debug)]
pub struct PPDrive;

impl PPDrive {
    /// add a new service to the manager
    pub async fn add(config: ServiceConfig, port: u16) -> AppResult<()> {
        let svc = Service::from(&config);
        svc.init()?;

        let resp = Self::send_request::<u8>(ServiceRequest::Add(config), port).await?;
        resp.log();

        Ok(())
    }

    /// cancel a service in the manager
    pub async fn cancel(id: u8, port: u16) -> AppResult<()> {
        let resp = Self::send_request::<()>(ServiceRequest::Cancel(id), port).await?;
        resp.log();

        Ok(())
    }

    pub async fn list(port: u16) -> AppResult<()> {
        let resp = Self::send_request::<Vec<ServiceInfo>>(ServiceRequest::List, port).await?;
        let list = resp.body();

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

    pub async fn create_client(port: u16, svc_id: u8, client_name: String) -> AppResult<()> {
        let resp =
            Self::send_request::<()>(ServiceRequest::CreateClient(svc_id, client_name), port)
                .await?;
        resp.log();

        Ok(())
    }

    /// check if ppdrive instance is running on a given port. we do this by attempting to read
    /// list of exisiting services. request failure most likely means ppdrive is not running.
    pub async fn check_status(port: u16) -> AppResult<()> {
        match Self::list(port).await {
            Ok(_) => tracing::info!("ppdrive is running on port {port}"),
            Err(_) => tracing::error!(
                "ppdrive is not running. run with 'ppdrive start' or check logs if starting fails."
            ),
        }

        Ok(())
    }

    pub async fn stop(port: u16) -> AppResult<()> {
        let resp = Self::send_request::<()>(ServiceRequest::Stop, port).await?;
        resp.log();

        Ok(())
    }

    /// send a command to manager's tcp connection
    async fn send_request<T: Encode + Decode<()>>(
        cmd: ServiceRequest,
        port: u16,
    ) -> AppResult<Response<T>> {
        match bincode::encode_to_vec(cmd, config::standard()) {
            Ok(data) => {
                let addr = Self::addr(port);
                let mut stream = TcpStream::connect(addr).await?;
                stream.write_all(&data).await?;

                stream.readable().await?;
                let mut buf = [0u8; 1024];

                stream.try_read(&mut buf)?;
                let resp = bincode::decode_from_slice(&buf, config::standard())?;

                Ok(resp.0)
            }
            Err(err) => Err(Error::InternalError(format!(
                "unable to encode service command: {err}"
            ))),
        }
    }

    fn addr(port: u16) -> String {
        format!("0.0.0.0:{}", port)
    }
}

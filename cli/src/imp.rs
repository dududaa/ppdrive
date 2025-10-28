use bincode::{Decode, Encode, config};

use ppd_shared::opts::{ClientDetails, Response, ServiceConfig, ServiceInfo, ServiceRequest};

use crate::errors::{AppResult, Error};
use ppdrive::plugin::service::Service;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

#[derive(Debug)]
pub struct PPDrive;

impl PPDrive {
    /// add a new service to the manager
    pub fn add(config: ServiceConfig, port: u16) -> AppResult<u8> {
        let svc = Service::from(&config);
        svc.init()?;

        let resp = Self::send_request::<u8>(ServiceRequest::Add(config.clone()), port)?;
        resp.log();

        tracing::info!("waiting to validate service startup...");
        std::thread::sleep(Duration::from_secs(2));

        match svc.connect() {
            Ok(_) => tracing::info!("service running with id {}", resp.body()),
            Err(err) => tracing::error!(
                "service fails to run: {err}\nPlease try \"ppdrive log\" for full details."
            ),
        }
        Ok(*resp.body())
    }

    /// cancel a service in the manager
    pub fn cancel(id: u8, port: u16) -> AppResult<()> {
        let resp = Self::send_request::<()>(ServiceRequest::Cancel(id), port)?;
        resp.log();

        Ok(())
    }

    pub fn list(port: u16) -> AppResult<()> {
        let resp = Self::send_request::<Vec<ServiceInfo>>(ServiceRequest::List, port)?;
        let list = resp.body();

        resp.log();
        if !list.is_empty() {
            println!(" id\t | port\t | type\t | auth-modes");
            for svc in list {
                let ServiceInfo {
                    id,
                    port,
                    auth_modes,
                    ty,
                } = svc;

                let modes: Vec<String> = auth_modes.iter().map(|m| format!("{m}")).collect();
                let modes: String = modes.join(", ");

                println!(" {id}\t | {port}\t | {ty}\t | {modes}")
            }
        } else {
            println!("no service running");
        }

        Ok(())
    }

    pub fn create_client(port: u16, svc_id: u8, client_name: String) -> AppResult<()> {
        let resp =
            Self::send_request::<Option<ClientDetails>>(ServiceRequest::CreateClient(svc_id, client_name), port)?;
        
        resp.log();
        if let Some(client) = resp.body() {
            println!("{client}");
        }

        Ok(())
    }

    pub fn refresh_client_token(port: u16, svc_id: u8, client_key: String) -> AppResult<()> {
        let resp =
            Self::send_request::<Option<String>>(ServiceRequest::RefreshClientToken(svc_id, client_key), port)?;
        
        resp.log();
        if let Some(token) = resp.body() {
            println!("{token}");
        }
        
        Ok(())
    }

    /// check if ppdrive instance is running on a given port. we do this by attempting to read
    /// list of exisiting services. request failure most likely means ppdrive is not running.
    pub fn check_status(port: u16) -> AppResult<()> {
        let addr = Self::addr(port);

        match TcpStream::connect(addr) {
            Ok(_) => tracing::info!("ppdrive is running on port {port}"),
            Err(_) => tracing::error!(
                "ppdrive is not running. run with 'ppdrive start' or check logs if starting fails."
            ),
        }

        Ok(())
    }

    pub fn stop(port: u16) -> AppResult<()> {
        let resp = Self::send_request::<()>(ServiceRequest::Stop, port)?;
        resp.log();

        Ok(())
    }

    /// send a command to manager's tcp connection
    fn send_request<T: Encode + Decode<()>>(
        cmd: ServiceRequest,
        port: u16,
    ) -> AppResult<Response<T>> {
        match bincode::encode_to_vec(cmd, config::standard()) {
            Ok(data) => {
                let addr = Self::addr(port);
                let mut stream = TcpStream::connect(addr)?;
                stream.write_all(&data)?;

                let mut buf = Vec::new();
                stream.read_to_end(&mut buf)?;
                let resp = bincode::decode_from_slice(&buf, config::standard())?;

                Ok(resp.0)
            }
            Err(err) => Err(Error::Internal(format!(
                "unable to encode service command: {err}"
            ))),
        }
    }

    fn addr(port: u16) -> String {
        format!("0.0.0.0:{port}")
    }
}

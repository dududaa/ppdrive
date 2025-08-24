use std::fmt::Display;

use crate::{AppResult, plugins::Plugin};
use clap::ValueEnum;
use libloading::Symbol;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Service {
    ty: ServiceType,
    port: u16,
}

impl Service {
    pub async fn start(&self) -> AppResult<()> {
        tracing::info!("starting server...");
        #[cfg(debug_assertions)]
        self.remove()?;

        self.preload()?;

        let filename = self.output()?;
        let port = self.port.clone();
        
        tracing::info!("{:?} plugin loaded...", self.output()?);
        tokio::spawn(async move {
            match Self::load(filename) {
                Ok(lib) => {
                    let start: Symbol<unsafe extern "C" fn(u16)> = unsafe { 
                        lib.get(b"start_server").expect("unable to load start_server Symbol") 
                    };
                    unsafe {
                        start(port);
                    }
                },
                Err(err) => tracing::error!("{err}")
            };
            
        });

        Ok(())
    }

    pub fn port(&self) -> &u16 {
        &self.port
    }
}

impl Default for Service {
    fn default() -> Self {
        Service {
            ty: ServiceType::default(),
            port: 5000,
        }
    }
}

impl Plugin for Service {
    fn package_name(&self) -> &'static str {
        match &self.ty {
            ServiceType::Rest => "ppd_rest",
            ServiceType::Grpc => "ppd_grpc",
        }
    }
}

pub struct ServiceBuilder {
    inner: Service,
}

impl ServiceBuilder {
    pub fn new(ty: ServiceType) -> Self {
        let inner = Service {
            ty,
            ..Default::default()
        };
        Self { inner }
    }

    pub fn port(mut self, port: u16) -> Self {
        self.inner.port = port;
        self
    }

    pub fn build(self) -> Service {
        self.inner
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Default, Debug)]
pub enum ServiceType {
    #[default]
    Rest,
    Grpc,
}

impl Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let o = match self {
            ServiceType::Rest => "rest",
            ServiceType::Grpc => "grpc",
        };

        writeln!(f, "{o}")
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Serialize, Deserialize)]
pub enum ServiceAuthMode {
    Client,
    User,
    Zero,
}

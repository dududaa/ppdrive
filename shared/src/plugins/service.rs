use std::{fmt::Display, path::PathBuf};

use crate::{AppResult, plugins::Plugin, tools::root_dir};
use clap::ValueEnum;
use libloading::Symbol;
use serde::{Deserialize, Serialize};

pub struct Service {
    ty: ServiceType,
    port: u16,
}

impl Service {
    pub async fn start(&self) -> AppResult<()> {
        #[cfg(debug_assertions)]
        self.remove()?;

        let lib = self.load()?;
        let start: Symbol<unsafe extern "C" fn(u16)> = unsafe { lib.get(b"start_server")? };
        
        unsafe {
            start(self.port);
        }

        Ok(())
    }

    fn plugin_name(&self) -> String {
        self.ty.to_string()
    }

    pub fn port(&self) -> &u16 {
        &self.port
    }
}

impl Default for Service  {
    fn default() -> Self {
        Service { ty: ServiceType::default(), port: 5000 }
    }
}

impl Plugin for Service {
    fn filename(&self) -> AppResult<PathBuf> {
        let mut n = self.plugin_name();
        n.push_str(Self::ext());

        let p = root_dir()?.join(n.to_lowercase());
        Ok(p)
    }

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

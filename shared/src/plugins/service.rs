use std::{fmt::Display, sync::Arc};

use crate::{AppResult, errors::Error, plugins::Plugin};
use bincode::{Decode, Encode, config};
use clap::{Args, ValueEnum};
use libloading::Symbol;

#[derive(Debug)]
pub struct Service {
    ty: ServiceType,
    port: u16,
}

impl Service {
    /// start a rest or grpc server
    pub fn start(&self, config: Arc<ServiceConfig>) -> AppResult<()> {
        tracing::info!("starting server...");
        #[cfg(debug_assertions)]
        self.remove()?;

        self.preload()?;
        let filename = self.output()?;
        let (cfg, len) = unsafe{config.into_raw()?};

        match Self::load(filename) {
            Ok(lib) => {
                let start: Symbol<unsafe extern "C" fn(*const u8, usize)> = unsafe {
                    lib.get(b"start_server")
                        .expect("unable to load start_server Symbol")
                };
                unsafe {
                    start(cfg, len);
                }
            }
            Err(err) => tracing::error!("{err}"),
        };

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

#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Default, Debug, Encode, Decode,
)]
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Encode, Decode)]
pub enum ServiceAuthMode {
    Client,
    Direct,
    Zero,
}

/// configuration for each service created.
#[derive(Debug, Args, Encode, Decode, Clone)]
pub struct ServiceBaseConfig {
    #[arg(long("db"), default_value_t=String::from("sqlite://db.sqlite"))]
    pub db_url: String,

    #[arg(long, default_value_t = 5000)]
    pub port: u16,

    #[arg(long("max-upload"), default_value_t = 10)]
    pub max_upload_size: usize,

    #[arg(long("allowed-origins"))]
    pub allowed_origins: Option<Vec<String>>,
}

/// authentication configuration for a service
#[derive(Debug, Args, Clone, Encode, Decode)]
pub struct ServiceAuthConfig {
    #[arg(long("auth-modes"))]
    pub modes: Vec<ServiceAuthMode>,

    #[arg(long, default_value_t = 900)]
    pub access_exp: i64,

    #[arg(long, default_value_t = 86400)]
    pub refresh_exp: i64,

    #[arg(long("auth-url"))]
    pub url: Option<String>,
}

#[derive(Encode, Decode, Clone)]
pub struct ServiceConfig {
    pub ty: ServiceType,
    pub base: ServiceBaseConfig,
    pub auth: ServiceAuthConfig,
}

impl ServiceConfig {
    /// make config ffi-safe
    pub unsafe fn into_raw(&self) -> AppResult<(*const u8, usize)> {
        let data = bincode::encode_to_vec(&self, config::standard())
            .map_err(|err| Error::ServerError(format!("unable to decode config: {err}")))?;
        let len = data.len();

        Ok((data.as_ptr(), len))
    }

    pub fn from_raw(data: &[u8]) -> AppResult<(Self, usize)> {
        let s = bincode::decode_from_slice::<ServiceConfig, _>(&data, config::standard())
            .map_err(|err| Error::ServerError(err.to_string()))?;
        Ok(s)
    }

    pub fn into_vec(self) -> AppResult<Vec<u8>> {
        let v = bincode::encode_to_vec(self, config::standard())
            .map_err(|err| Error::ServerError(err.to_string()))?;
        Ok(v)
    }
}

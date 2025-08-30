use std::{fmt::Display, sync::Arc};

use crate::{
    AppResult,
    errors::Error,
    plugins::{HasDependecies, Plugin, router::ServiceRouter},
};
use bincode::{Decode, Encode, config};
use clap::{Args, ValueEnum};
use libloading::Symbol;

#[derive(Debug)]
pub struct Service<'a> {
    ty: &'a ServiceType,
    port: &'a u16,
    modes: &'a [ServiceAuthMode],
}

impl<'a> Service<'a> {
    /// start a rest or grpc service
    pub fn start(&self, config: Arc<ServiceConfig>) -> AppResult<()> {
        let filename = self.output()?;
        let (cfg, len) = unsafe { config.into_raw()? };

        let lib = self.load(filename)?;
        let start: Symbol<unsafe extern "C" fn(*const u8, usize)> = unsafe {
            lib.get(b"start_server")
                .expect("unable to load start_server Symbol")
        };

        unsafe {
            start(cfg, len);
        }

        Ok(())
    }

    pub fn port(&self) -> &u16 {
        &self.port
    }

    pub fn modes(&self) -> &[ServiceAuthMode] {
        self.modes
    }
}

impl<'a> From<&'a ServiceConfig> for Service<'a> {
    fn from(value: &'a ServiceConfig) -> Self {
        Self {
            ty: &value.ty,
            port: &value.base.port,
            modes: &value.auth.modes,
        }
    }
}

impl<'a> Plugin for Service<'a> {
    fn package_name(&self) -> &'static str {
        match &self.ty {
            ServiceType::Rest => "ppd_rest",
            ServiceType::Grpc => "ppd_grpc",
        }
    }
}

impl<'a> HasDependecies for Service<'a> {
    fn dependecies(&self) -> Vec<Box<dyn Plugin>> {
        let routers: Vec<Box<dyn Plugin>> = self
            .modes
            .iter()
            .map(|mode| {
                Box::new(ServiceRouter {
                    svc_type: *self.ty,
                    auth_mode: mode.clone(),
                }) as Box<dyn Plugin>
            })
            .collect();

        routers
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

#[derive(Encode, Decode, Clone, Debug)]
pub struct ServiceConfig {
    pub ty: ServiceType,
    pub base: ServiceBaseConfig,
    pub auth: ServiceAuthConfig,
}

impl ServiceConfig {
    /// make ServiceConfig ffi-safe
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

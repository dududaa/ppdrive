use std::{fmt::Display, sync::Arc};

use crate::{
    AppResult,
    plugins::{HasDependecies, Plugin, router::ServiceRouter},
};
use bincode::{Decode, Encode};
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
    pub fn start(&self, config: ServiceConfig) -> AppResult<()> {
        let filename = self.output()?;
        let cfg_ptr = Arc::new(config);
        let cfg_raw = Arc::into_raw(cfg_ptr);

        let lib = self.load(filename)?;
        let start: Symbol<unsafe extern "C" fn(*const ServiceConfig)> = unsafe {
            lib.get(b"start_svc")
                .expect("unable to load start_server Symbol")
        };

        unsafe {
            start(cfg_raw);
        }

        Ok(())
    }

    pub fn ty(&self) -> &ServiceType {
        &self.ty
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
#[derive(Debug, Args, Encode, Decode, Clone, Default)]
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
#[derive(Debug, Args, Clone, Encode, Decode, Default)]
pub struct ServiceAuthConfig {
    #[arg(long("auth-modes"), value_enum, default_values = ["client"])]
    pub modes: Vec<ServiceAuthMode>,

    #[arg(long, default_value_t = 900)]
    pub access_exp: i64,

    #[arg(long, default_value_t = 86400)]
    pub refresh_exp: i64,

    #[arg(long("auth-url"))]
    pub url: Option<String>,
}

#[derive(Encode, Decode, Clone, Debug, Default)]
pub struct ServiceConfig {
    pub ty: ServiceType,
    pub base: ServiceBaseConfig,
    pub auth: ServiceAuthConfig,
}

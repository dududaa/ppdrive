use std::fmt::Display;

use crate::{errors::Error, plugins::Plugin, AppResult};
use bincode::{config, Decode, Encode};
use clap::{Args, ValueEnum};
use libloading::Symbol;

#[derive(Debug)]
pub struct Service {
    ty: ServiceType,
    port: u16,
}

impl Service {
    /// start a rest or grpc server
    pub async fn start(&self, config: ServiceConfig) -> AppResult<()> {
        tracing::info!("starting server...");
        #[cfg(debug_assertions)]
        self.remove()?;

        self.preload()?;
        let filename = self.output()?;

        match Self::load(filename) {
            Ok(lib) => {
                
                let start: Symbol<unsafe extern "C" fn(*const u8, usize)> = unsafe {
                    lib.get(b"start_server")
                        .expect("unable to load start_server Symbol")
                };
                unsafe {
                    let (cfg_data, cfg_len) = config.into_raw()?;
                    start(cfg_data, cfg_len);
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
    User,
    Zero,
}

/// configuration for each service created.
#[derive(Debug, Args, Encode, Decode, Clone)]
pub struct ServiceBaseConfig {
    pub ty: ServiceType,
    pub db_url: String,
    pub port: u16,
    pub max_upload_size: usize,
    pub allowed_origins: Option<Vec<String>>,
}

impl Default for ServiceBaseConfig {
    fn default() -> Self {
        ServiceBaseConfig {
            db_url: "sqlite://db.sqlite".to_string(),
            port: 5000,
            max_upload_size: 10,
            allowed_origins: None,
            ty: ServiceType::Rest,
        }
    }
}

/// authentication configuration for a service
#[derive(Debug, Args, Clone, Encode, Decode)]
pub struct ServiceAuthConfig {
    pub modes: Vec<ServiceAuthMode>,
    pub access_exp: Option<i64>,
    pub refresh_exp: Option<i64>,
    pub url: Option<String>
}

impl Default for ServiceAuthConfig {
    fn default() -> Self {
        ServiceAuthConfig {
            modes: vec![],
            access_exp: Some(900),
            refresh_exp: Some(86400),
            url: None
        }
    }
}

#[derive(Encode, Decode, Default, Clone)]
pub struct ServiceConfig {
    pub base: ServiceBaseConfig,
    pub auth: ServiceAuthConfig,
}

impl ServiceConfig {
    pub fn base(mut self, base: Option<ServiceBaseConfig>) -> Self {
        self.base = base.unwrap_or_default();
        self
    }

    pub fn auth(mut self, auth: Option<ServiceAuthConfig>) -> Self {
        self.auth = auth.unwrap_or_default();
        self
    }

    pub fn service_type(mut self, ty: ServiceType) -> Self {
        self.base.ty = ty;
        self
    }

    /// make config ffi-safe
    pub unsafe  fn into_raw(self) -> AppResult<(*const u8, usize)> {
        let data = bincode::encode_to_vec(self, config::standard()).map_err(|err| Error::ServerError(format!("unable to decode config: {err}")))?;
        let len = data.len();

        Ok((data.as_ptr(), len))
    }
}

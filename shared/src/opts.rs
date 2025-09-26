use bincode::{Decode, Encode};
use clap::{Args, ValueEnum};
use constants::*;
use std::fmt::Display;

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

#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Encode, Decode, Default,
)]
pub enum ServiceAuthMode {
    #[default]
    Client,
    Direct,
    Zero,
}

/// configuration for each service created.
#[derive(Debug, Args, Encode, Decode, Clone)]
pub struct ServiceBaseConfig {
    /// url of database to be used by ppdrive.
    #[arg(long("db-url"), default_value_t = DEFAULT_DB_URL.to_string())]
    pub db_url: String,

    /// port on which to run service
    #[arg(long, default_value_t = DEFAULT_SERVICE_PORT)]
    pub port: u16,

    /// maximum request content size for this service (MB).
    #[arg(long("max-upload"), default_value_t = DEFAULT_MAX_UPLOAD)]
    pub max_upload_size: usize,

    /// urls allowed by CORS policy for this service. if this is not set, we allow all url (*).
    #[arg(long("allowed-origins"))]
    pub allowed_origins: Option<Vec<String>>,
}

impl Default for ServiceBaseConfig {
    fn default() -> Self {
        Self {
            db_url: DEFAULT_DB_URL.to_string(),
            port: DEFAULT_SERVICE_PORT,
            max_upload_size: DEFAULT_MAX_UPLOAD,
            allowed_origins: None,
        }
    }
}

/// authentication configuration for a service
#[derive(Debug, Args, Clone, Encode, Decode)]
pub struct ServiceAuthConfig {
    /// authentication modes for the service.
    #[arg(long("auth-modes"), value_enum, default_values = ["client"])]
    pub modes: Vec<ServiceAuthMode>,

    /// JWT access token expiration for the service (seconds).
    #[arg(long, default_value_t = DEFAULT_ACCESS_TOKEN_EXP)]
    pub access_exp: i64,

    /// JWT refresh token expiration for the service (seconds).
    #[arg(long, default_value_t = DEFAULT_REFRESH_TOKEN_EXP)]
    pub refresh_exp: i64,

    /// external url to be used for authentication.
    #[arg(long("auth-url"))]
    pub url: Option<String>,
}

impl Default for ServiceAuthConfig {
    fn default() -> Self {
        Self {
            modes: vec![ServiceAuthMode::Client],
            access_exp: DEFAULT_ACCESS_TOKEN_EXP,
            refresh_exp: DEFAULT_REFRESH_TOKEN_EXP,
            url: None,
        }
    }
}

#[derive(Encode, Decode, Clone, Debug, Default)]
pub struct ServiceConfig {
    pub ty: ServiceType,
    pub base: ServiceBaseConfig,
    pub auth: ServiceAuthConfig,
    pub auto_install: bool,
}

mod constants {
    pub const DEFAULT_DB_URL: &'static str = "sqlite://db.sqlite";
    pub const DEFAULT_SERVICE_PORT: u16 = 5000;
    pub const DEFAULT_MAX_UPLOAD: usize = 10;
    pub const DEFAULT_ACCESS_TOKEN_EXP: i64 = 900;
    pub const DEFAULT_REFRESH_TOKEN_EXP: i64 = 86400;
}

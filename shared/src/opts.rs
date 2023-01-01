
use bincode::{Decode, Encode};
use clap::{Args, ValueEnum};
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Encode, Decode, Default)]
pub enum ServiceAuthMode {
    #[default]
    Client,
    Direct,
    Zero,
}

/// configuration for each service created.
#[derive(Debug, Args, Encode, Decode, Clone, Default)]
pub struct ServiceBaseConfig {
    /// url of database to be used by ppdrive.
    #[arg(long("db"), default_value_t=String::from("sqlite://db.sqlite"))]
    pub db_url: String,

    /// port on which to run service
    #[arg(long, default_value_t = 5000)]
    pub port: u16,

    /// maximum request content size for this service (MB).
    #[arg(long("max-upload"), default_value_t = 10)]
    pub max_upload_size: usize,

    /// urls allowed by CORS policy for this service. if this is not set, we allow all url (*).
    #[arg(long("allowed-origins"))]
    pub allowed_origins: Option<Vec<String>>,
}

/// authentication configuration for a service
#[derive(Debug, Args, Clone, Encode, Decode, Default)]
pub struct ServiceAuthConfig {
    /// authentication modes for the service.
    #[arg(long("auth-modes"), value_enum, default_values = ["client"])]
    pub modes: Vec<ServiceAuthMode>,

    /// JWT access token expiration for the service (seconds).
    #[arg(long, default_value_t = 900)]
    pub access_exp: i64,

    /// JWT refresh token expiration for the service (seconds).
    #[arg(long, default_value_t = 86400)]
    pub refresh_exp: i64,

    /// external url to be used for authentication.
    #[arg(long("auth-url"))]
    pub url: Option<String>,
}

#[derive(Encode, Decode, Clone, Debug, Default)]
pub struct ServiceConfig {
    pub ty: ServiceType,
    pub base: ServiceBaseConfig,
    pub auth: ServiceAuthConfig,
}

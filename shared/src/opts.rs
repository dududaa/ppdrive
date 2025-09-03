
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

use std::fmt::Display;

use ppdrive_rest::errors::RestError;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    VarError(std::env::VarError),
    ServerError(RestError),
    IOError(std::io::Error),
    CommandError(String),
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CliError::*;

        match self {
            VarError(err) => write!(f, "{err}"),
            IOError(err) => write!(f, "{err}"),
            ServerError(err) => write!(f, "{err}"),
            CommandError(err) => write!(f, "{err}"),
        }
    }
}

impl From<std::env::VarError> for CliError {
    fn from(value: std::env::VarError) -> Self {
        CliError::VarError(value)
    }
}

impl From<RestError> for CliError {
    fn from(value: RestError) -> Self {
        CliError::ServerError(value)
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        CliError::IOError(value)
    }
}

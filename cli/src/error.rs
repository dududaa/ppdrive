use std::fmt::Display;

use ppdrive_core::errors::CoreError;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
pub enum CliError {
    VarError(std::env::VarError),
    DatabaseError(CoreError),
    IOError(std::io::Error),
    ParseError(String),
    CommandError(String),
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CliError::*;

        match self {
            VarError(err) => write!(f, "{err}"),
            ParseError(err) => write!(f, "{err}"),
            IOError(err) => write!(f, "{err}"),
            DatabaseError(err) => write!(f, "{err}"),
            CommandError(err) => write!(f, "{err}"),
        }
    }
}

impl From<std::env::VarError> for CliError {
    fn from(value: std::env::VarError) -> Self {
        CliError::VarError(value)
    }
}

impl From<CoreError> for CliError {
    fn from(value: CoreError) -> Self {
        CliError::DatabaseError(value)
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        CliError::IOError(value)
    }
}

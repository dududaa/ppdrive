use std::fmt::Display;

use modeller::prelude::Error as ModellerError;
use rbatis::Error as ExecError;
use ppd_shared::errors::Error as SharedError;

#[derive(Debug)]
pub enum Error {
    MigrationError(ModellerError),
    ExecError(ExecError),
    ParseError(String),
    NotFound(String),
    PermissionError(String),
    ServerError(String)
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            MigrationError(err) => write!(f, "{err}"),
            ExecError(err) => write!(f, "{err}"),
            ParseError(msg) => write!(f, "{msg}"),
            NotFound(msg) => write!(f, "{msg}"),
            PermissionError(msg) => write!(f, "{msg}"),
            ServerError(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<ModellerError> for Error {
    fn from(value: ModellerError) -> Self {
        Error::MigrationError(value)
    }
}

impl From<ExecError> for Error {
    fn from(value: ExecError) -> Self {
        Error::ExecError(value)
    }
}

impl From<SharedError> for Error {
    fn from(value: SharedError) -> Self {
        Error::ServerError(value.to_string())
    }
}

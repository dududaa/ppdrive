use bincode::error::{DecodeError, EncodeError};
use ppd_shared::errors::Error as SharedError;
use std::fmt::Display;

pub type AppResult<T> = Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    VarError(std::env::VarError),
    IOError(std::io::Error),
    InternalError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            VarError(err) => write!(f, "{err}"),
            IOError(err) => write!(f, "{err}"),
            InternalError(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<std::env::VarError> for Error {
    fn from(value: std::env::VarError) -> Self {
        Error::VarError(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IOError(value)
    }
}

impl From<SharedError> for Error {
    fn from(value: SharedError) -> Self {
        Error::InternalError(value.to_string())
    }
}

impl From<DecodeError> for Error {
    fn from(value: DecodeError) -> Self {
        Error::InternalError(value.to_string())
    }
}

impl From<EncodeError> for Error {
    fn from(value: EncodeError) -> Self {
        Error::InternalError(value.to_string())
    }
}

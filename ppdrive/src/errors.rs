use bincode::error::{DecodeError, EncodeError};
use ppd_shared::errors::Error as SharedError;
use handlers::errors::HandlerError;
use tokio::task::JoinError;
use std::fmt::Display;

pub type AppResult<T> = Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Env(std::env::VarError),
    IO(std::io::Error),
    Internal(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            Env(err) => write!(f, "{err}"),
            IO(err) => write!(f, "{err}"),
            Internal(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<std::env::VarError> for Error {
    fn from(value: std::env::VarError) -> Self {
        Error::Env(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IO(value)
    }
}

impl From<SharedError> for Error {
    fn from(value: SharedError) -> Self {
        Error::Internal(value.to_string())
    }
}

impl From<HandlerError> for Error {
    fn from(value: HandlerError) -> Self {
        Error::Internal(value.to_string())
    }
}

impl From<JoinError> for Error {
    fn from(value: JoinError) -> Self {
        Error::Internal(value.to_string())
    }
}

impl From<DecodeError> for Error {
    fn from(value: DecodeError) -> Self {
        Error::Internal(value.to_string())
    }
}

impl From<EncodeError> for Error {
    fn from(value: EncodeError) -> Self {
        Error::Internal(value.to_string())
    }
}

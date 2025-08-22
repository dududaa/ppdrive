use std::{fmt::Display, io};
use libloading::Error as LibLoadError;

#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    LibLoadError(LibLoadError),
    ServerError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            IOError(err) => write!(f, "{err}"),
            LibLoadError(err) => write!(f, "{err}"),
            ServerError(msg) => write!(f, "{msg}")
        }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IOError(value)
    }
}

impl From<LibLoadError> for Error  {
    fn from(value: LibLoadError) -> Self {
        Error::LibLoadError(value)
    }
}

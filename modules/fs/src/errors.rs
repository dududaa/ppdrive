use std::fmt::Display;

use ppd_bk::errors::Error as DatabaseError;
use tokio::io::Error as IOError;

#[derive(Debug)]
pub enum Error {
    DatabaseError(DatabaseError),
    IOError(IOError),
    ServerError(String),
    PermissionError(String),
    NotFound(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        match self {
            DatabaseError(err) => write!(f, "{err}"),
            IOError(err) => write!(f, "{err}"),
            ServerError(msg) => write!(f, "{msg}"),
            PermissionError(msg) => write!(f, "{msg}"),
            NotFound(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<DatabaseError> for Error {
    fn from(value: DatabaseError) -> Self {
        Error::DatabaseError(value)
    }
}

impl From<IOError> for Error {
    fn from(value: IOError) -> Self {
        Error::IOError(value)
    }
}

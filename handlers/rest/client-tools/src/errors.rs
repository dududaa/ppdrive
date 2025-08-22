use std::fmt::Display;
use chacha20poly1305::Error as XError;

use ppd_bk::errors::Error as DBError;

pub enum Error {
    AuthorizationError(String),
    DBError(DBError),
    XError(XError)
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        
        match self {
            AuthorizationError(msg) => write!(f, "{msg}"),
            DBError(err) => write!(f, "{err}"),
            XError(err) => write!(f, "{err}"),
        }
    }
}

impl From<DBError> for Error {
    fn from(value: DBError) -> Self {
        Error::DBError(value)
    }
}

impl From<XError> for Error {
    fn from(value: XError) -> Self {
        Error::XError(value)
    }
}
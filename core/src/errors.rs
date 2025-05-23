use std::fmt::Display;

pub type DbError = rbs::Error;
pub type IoError = std::io::Error;

#[derive(Debug)]
pub enum CoreError {
    DbError(DbError),
    IoError(IoError),
    ParseError(String),
    ServerError(String),
    PermissionError(String),
    MigrationError(sqlx::Error),
}

impl Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreError::DbError(err) => write!(f, "{err}"),
            CoreError::IoError(err) => write!(f, "{err}"),
            CoreError::ParseError(err) => write!(f, "{err}"),
            CoreError::ServerError(err) => write!(f, "{err}"),
            CoreError::PermissionError(err) => write!(f, "{err}"),
            CoreError::MigrationError(err) => write!(f, "{err}"),
        }
    }
}

impl From<DbError> for CoreError {
    fn from(value: DbError) -> Self {
        CoreError::DbError(value)
    }
}

impl From<IoError> for CoreError {
    fn from(value: IoError) -> Self {
        CoreError::IoError(value)
    }
}

impl From<sqlx::Error> for CoreError {
    fn from(value: sqlx::Error) -> Self {
        CoreError::MigrationError(value)
    }
}

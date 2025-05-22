use errors::CoreError;

pub mod errors;
mod fs;
pub mod models;
pub mod options;

pub(self) type CoreResult<T> = Result<T, CoreError>;

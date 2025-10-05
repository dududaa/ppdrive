
#[cfg(feature = "rbatis")]
pub use rbatis::RBatis;

#[cfg(feature = "prelude")]
pub use errors::Error;

#[cfg(feature = "prelude")]
pub mod db;
#[cfg(feature = "prelude")]
pub mod models;

#[cfg(feature = "prelude")]
pub mod errors;

#[cfg(feature = "prelude")]
pub mod validators;


type DBResult<T> = Result<T, Error>;
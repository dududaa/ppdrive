//! function used for creating services and service handlers (routers)

use crate::errors::HandlerError;

pub mod errors;

pub mod prelude;

#[cfg(feature = "jwt")]
pub mod jwt;

#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "tools")]
pub mod tools;

#[cfg(feature = "rest")]
pub mod rest;

#[cfg(feature = "db")]
pub use ppd_bk::db;

pub type HandlerResult<T> = Result<T, HandlerError>;

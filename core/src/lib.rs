//! function used for creating services and service handlers (routers)

use std::sync::Arc;

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

/// a router type retuned from loading a service router library over FFI
pub type RouterFFI<T> = Arc<T>;

/// we use this as a uniform signature builder for all router libraries so that if router symbol
/// signature needs to change, we make the changes here.
pub fn router_symbol_builder<T>(router: T) -> RouterFFI<T> {
    Arc::new(router)
}
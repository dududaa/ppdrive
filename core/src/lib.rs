//! function used for creating services and service handlers (routers)

use std::sync::Arc;
use tokio::runtime::Runtime;

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
use ppd_shared::opts::internal::ServiceConfig;

pub type HandlerResult<T> = Result<T, HandlerError>;

/// a router type retuned from loading a service router library over FFI
pub type RouterFFI<T> = Box<T>;

/// we use this as a uniform signature builder for all router libraries so that if router symbol
/// signature needs to change, we make the changes here.
pub fn router_symbol_builder<F, T>(config: Arc<ServiceConfig>, callback: F) -> RouterFFI<T>
where
    F: FnOnce(Arc<ServiceConfig>) -> T,
{
    let router = callback(config);
    Box::new(router)
}

/// Create a new tokio [Runtime] and run the provided `future` in [Runtime::block_on].
pub fn runtime_wrapper<F: Future>(future: F) -> F::Output {
    let rt = Runtime::new().expect("unable to start runtime");
    rt.block_on(future)
}
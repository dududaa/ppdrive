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

use async_ffi::{FfiFuture, FutureExt};
#[cfg(feature = "db")]
pub use ppd_bk::db;
use ppd_shared::opts::internal::ServiceConfig;
use tokio::runtime::Runtime;

pub type HandlerResult<T> = Result<T, HandlerError>;

/// a router type retuned from loading a service router library over FFI
pub type RouterFFI<T> = FfiFuture<Arc<T>>;

/// we use this as a uniform signature builder for all router libraries so that if router symbol
/// signature needs to change, we make the changes here.
pub fn router_symbol_builder<F, T>(config: Arc<ServiceConfig>, callback: F) -> RouterFFI<T>
where
    F: Send + 'static + AsyncFnOnce(Arc<ServiceConfig>) -> T,
    T: Send + Sync + 'static,
{
    async move {
        let rt = Runtime::new().expect("unable to create router runtime");
        rt.block_on(async move {
            let router = callback(config).await;
            Arc::new(router)
        })
    }
    .into_ffi()
}

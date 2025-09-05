//! implementation of [Plugin](crate::plugins::Plugin) for routers loaded for a [Service](super::service::Service),
//! depending `config.server.auth_type` parameter.

use std::str;

use axum::Router;
use libloading::{Library, Symbol};
use ppd_shared::{plugin::Plugin, opts::{ServiceAuthMode, ServiceType}};

use crate::{HandlerResult, prelude::state::HandlerState};

#[derive(Default)]
pub struct ServiceRouter {
    pub svc_type: ServiceType,
    pub auth_mode: ServiceAuthMode,
}

impl ServiceRouter {
    pub fn get(&self, max_upload_size: usize) -> HandlerResult<SharedRouter> {
        let filename = self.output()?;
        let lib = self.load(filename)?;

        let ptr = unsafe {
            let load_router: Symbol<fn(usize) -> *mut Router<HandlerState>> = lib.get(b"load_router")?;
            load_router(max_upload_size)
        };
        
        let router = SharedRouter{ ptr, lib };
        Ok(router)
    }
}

impl Plugin for ServiceRouter {
    fn package_name(&self) -> &'static str {
        use ServiceAuthMode::*;
        use ServiceType::*;

        match self.svc_type {
            Rest => match self.auth_mode {
                Client => "rest_client",
                _ => unimplemented!("loading plugin for this auth_mode is not supported"),
            },
            Grpc => unimplemented!("loading plugin for a grpc server is not implemented."),
        }
    }
}

/// ffi-safe router shared by dynamic libraries
pub struct SharedRouter {
    ptr: *mut Router<HandlerState>,
    lib: Library
}

impl SharedRouter {
    pub fn as_ref(&self) -> &Router<HandlerState> {
        unsafe { &*self.ptr }
    }
}

impl Drop for SharedRouter  {
    fn drop(&mut self) {
        unsafe {
            let free_router = self.lib.get::<fn(*mut Router<HandlerState>)>(b"free_router");
            match free_router {
                Ok(call) => call(self.ptr),
                Err(err) => println!("unable to drop shared router {err}")
            }
        }
    }
}
//! implementation of [Plugin](crate::plugins::Plugin) for routers loaded for a [Service](super::service::Service),
//! depending `config.server.auth_type` parameter.

use std::{str, sync::Arc};

use axum::Router;
use libloading::Symbol;
use ppd_shared::{
    opts::{ServiceAuthMode, ServiceConfig, ServiceType},
    plugin::Plugin,
};

use crate::{HandlerResult, prelude::state::HandlerState};

type RouterType = Router<HandlerState>;
type RawRouterType = *mut RouterType;

#[derive(Default)]
pub struct Routers {
    svc_type: ServiceType,
    svc_max_upload: usize,
    auth_modes: Vec<ServiceAuthMode>,
    pub client: RawRouterType,
    admin: RawRouterType,
    direct: RawRouterType,
    zero: RawRouterType,
}

impl Routers {
    pub fn client(&self) -> RouterType {
        Self::get_router(self.client)
    }

    pub fn load(mut self) -> HandlerResult<Self> {
        for mode in &self.auth_modes {
            let rtr = ServiceRouter {
                svc_type: self.svc_type.clone(),
                auth_mode: *mode,
            };

            let ptr = rtr.get(self.svc_max_upload)?;
            match mode {
                ServiceAuthMode::Client => self.client = ptr,
                _ => unimplemented!(),
            }
        }

        Ok(self)
    }

    fn get_router(ptr: RawRouterType) -> RouterType {
        if ptr.is_null() {
            Router::new()
        } else {
            (unsafe { &*ptr }).clone()
        }
    }

    fn drop_router(ptr: RawRouterType) {
        if !ptr.is_null() {
            let _ = unsafe { Box::from_raw(ptr) };
        }
    }
}

impl From<Arc<ServiceConfig>> for Routers {
    fn from(value: Arc<ServiceConfig>) -> Self {
        Self {
            svc_type: value.ty,
            svc_max_upload: value.base.max_upload_size,
            auth_modes: value.auth.modes.clone(),
            ..Default::default()
        }
    }
}

impl Drop for Routers {
    fn drop(&mut self) {
        Self::drop_router(self.admin);
        Self::drop_router(self.client);
        Self::drop_router(self.zero);
        Self::drop_router(self.direct);
    }
}

#[derive(Default)]
pub struct ServiceRouter {
    pub svc_type: ServiceType,
    pub auth_mode: ServiceAuthMode,
}

impl ServiceRouter {
    pub fn get(&self, max_upload_size: usize) -> HandlerResult<RawRouterType> {
        let filename = self.output()?;
        let lib = self.load(filename)?;

        let rtr = unsafe {
            let load_router: Symbol<fn(usize) -> RawRouterType> = lib.get(b"load_router")?;
            load_router(max_upload_size)
        };

        Ok(rtr)
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

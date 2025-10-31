//! implementation of [Plugin](crate::plugins::Plugin) for routers loaded for a [Service](super::service::Service),
//! depending `config.server.auth_type` parameter.

use std::{str, sync::Arc};

use axum::Router;
use libloading::{Library, Symbol};
use ppd_shared::{
    opts::{ServiceAuthMode, ServiceConfig, ServiceType},
    plugin::Plugin,
};

use crate::{HandlerResult, prelude::state::HandlerState};

type RouterType = Router<HandlerState>;
type RawRouterType = *mut RouterType;

#[allow(dead_code)]
#[derive(Default)]
pub struct RouterLoader {
    ptr: RawRouterType,
    lib: Option<Library>,
}

#[derive(Default)]
pub struct Routers {
    config: Arc<ServiceConfig>,
    client: RouterLoader,
    admin: RouterLoader,
    direct: RouterLoader,
    zero: RouterLoader,
}

impl Routers {
    pub fn client(&self) -> RouterType {
        Self::get_router(&self.client)
    }

    pub fn direct(&self) -> RouterType {
        Self::get_router(&self.direct)
    }

    pub fn load(mut self) -> HandlerResult<Self> {
        let config = self.config.clone();
        let modes = &config.auth.modes;
        let svc_type = config.ty;

        for mode in modes {
            let router = ServiceRouter {
                svc_type: svc_type.clone(),
                auth_mode: *mode,
            };

            let ptr = router.get(self.config.clone())?;
            match mode {
                ServiceAuthMode::Client => self.client = ptr,
                ServiceAuthMode::Direct => self.client = ptr,
                _ => unimplemented!(),
            }
        }

        Ok(self)
    }

    fn get_router(ld: &RouterLoader) -> RouterType {
        let ptr = ld.ptr;

        if ptr.is_null() {
            Router::new()
        } else {
            (unsafe { &*ptr }).clone()
        }
    }

    fn drop_router(ld: &RouterLoader) {
        let ptr = ld.ptr;

        if !ptr.is_null() {
            let _ = unsafe { Box::from_raw(ptr) };
        }
    }
}

impl From<Arc<ServiceConfig>> for Routers {
    fn from(value: Arc<ServiceConfig>) -> Self {
        let mut rts = Routers::default();
        rts.config = value;

        rts
    }
}

impl Drop for Routers {
    fn drop(&mut self) {
        Self::drop_router(&self.admin);
        Self::drop_router(&self.client);
        Self::drop_router(&self.zero);
        Self::drop_router(&self.direct);
    }
}

#[derive(Default)]
pub struct ServiceRouter {
    pub svc_type: ServiceType,
    pub auth_mode: ServiceAuthMode,
}

impl ServiceRouter {
    pub fn get(&self, config: Arc<ServiceConfig>) -> HandlerResult<RouterLoader> {
        let filename = self.output_name()?;
        let lib = self.load(filename)?;

        let ptr = unsafe {
            let load_router: Symbol<fn(*const ServiceConfig) -> RawRouterType> =
                lib.get(&self.symbol_name())?;
            load_router(Arc::into_raw(config))
        };

        Ok(RouterLoader {
            ptr,
            lib: Some(lib),
        })
    }
}

impl Plugin for ServiceRouter {
    fn package_name(&self) -> &'static str {
        use ServiceAuthMode::*;
        use ServiceType::*;

        match self.svc_type {
            Rest => match self.auth_mode {
                Client => "rest-client",
                Direct => "rest-direct",
                _ => unimplemented!("loading plugin for this auth_mode is not supported"),
            },
            Grpc => unimplemented!("loading plugin for a grpc server is not implemented."),
        }
    }
}

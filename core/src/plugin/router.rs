//! implementation of [Plugin](crate::plugins::Plugin) for routers loaded for a [Service](super::service::Service),
//! depending `config.server.auth_type` parameter.

use std::{str, sync::Arc};

use libloading::{Library, Symbol};
use ppd_shared::{
    opts::internal::{ServiceAuthMode, ServiceConfig, ServiceType},
    plugin::Plugin,
};

use crate::{HandlerResult, RouterFFI};

#[allow(dead_code)]
#[derive(Default)]
pub struct RouterLoader<T: Default + Clone> {
    ptr: Arc<T>,
    lib: Option<Library>,
}

#[derive(Default)]
pub struct Routers<T: Default + Clone> {
    config: Arc<ServiceConfig>,
    client: RouterLoader<T>,
    direct: RouterLoader<T>,
    // admin: RouterLoader<T>,
    // zero: RouterLoader<T>,
}

impl<T: Default + Clone> Routers<T> {
    pub fn client(&self) -> T {
        Self::get_router(&self.client)
    }

    pub fn direct(&self) -> T {
        Self::get_router(&self.direct)
    }

    pub async fn load(mut self) -> HandlerResult<Self> {
        let config = self.config.clone();
        let modes = &config.auth.modes;
        let svc_type = config.ty;

        for mode in modes {
            let router = ServiceRouter {
                svc_type,
                auth_mode: *mode,
            };

            let ptr = router.get(self.config.clone()).await?;
            match mode {
                ServiceAuthMode::Client => self.client = ptr,
                ServiceAuthMode::Direct => self.direct = ptr,
                _ => unimplemented!(),
            }
        }

        Ok(self)
    }

    fn get_router(ld: &RouterLoader<T>) -> T {
        let ptr = ld.ptr.clone();
        (&*ptr).clone()
    }
}

impl<T: Default + Clone> From<Arc<ServiceConfig>> for Routers<T> {
    fn from(value: Arc<ServiceConfig>) -> Self {
        let mut rts = Routers::default();
        rts.config = value;

        rts
    }
}

#[derive(Default)]
pub struct ServiceRouter {
    pub svc_type: ServiceType,
    pub auth_mode: ServiceAuthMode,
}

impl ServiceRouter {
    pub async fn get<T: Default + Clone>(
        &self,
        config: Arc<ServiceConfig>,
    ) -> HandlerResult<RouterLoader<T>> {
        let filename = self.output_name()?;
        let lib = self.load(filename)?;

        let ptr = unsafe {
            let load_router: Symbol<fn(Arc<ServiceConfig>) -> RouterFFI<T>> =
                lib.get(&self.symbol_name())?;
            
            load_router(config).await
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

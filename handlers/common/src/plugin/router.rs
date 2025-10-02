//! implementation of [Plugin](crate::plugins::Plugin) for routers loaded for a [Service](super::service::Service),
//! depending `config.server.auth_type` parameter.

use std::str;

use libloading::Symbol;
use ppd_shared::{
    opts::{ServiceAuthMode, ServiceType},
    plugin::Plugin,
};

use crate::HandlerResult;

#[derive(Default)]
pub struct ServiceRouter {
    pub svc_type: ServiceType,
    pub auth_mode: ServiceAuthMode,
}

impl ServiceRouter {
    pub fn get<T>(&self, max_upload_size: usize) -> HandlerResult<T> {
        let filename = self.output()?;
        let lib = self.load(filename)?;

        let ptr = unsafe {
            let load_router: Symbol<fn(usize) -> T> = lib.get(b"load_router")?;
            load_router(max_upload_size)
        };

        Ok(ptr)
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

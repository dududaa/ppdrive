//! implementation of [Plugin](crate::plugins::Plugin) for routers loaded for a [Service](super::service::Service),
//! depending `config.server.auth_type` parameter.

use libloading::Symbol;

use crate::{
    plugins::{
        service::{ServiceAuthMode, ServiceType}, Plugin
    }, AppResult
};

pub struct ServiceRouter {
    pub svc_type: ServiceType,
    pub auth_mode: ServiceAuthMode,
}

impl ServiceRouter {
    pub fn get<T>(&self, max_upload_size: usize) -> AppResult<Box<T>> {

        #[cfg(debug_assertions)]
        self.remove()?;

        let filename = self.output()?;
        let lib = self.load(filename)?;
        
        let load_router: Symbol<unsafe extern "C" fn(usize) -> *mut T> =
            unsafe { lib.get(b"load_router")? };

        let router = unsafe { 
            let raw = load_router(max_upload_size);
            Box::from_raw(raw)
        };

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

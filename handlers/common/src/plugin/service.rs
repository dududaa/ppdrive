use std::sync::Arc;

use super::router::ServiceRouter;
use crate::HandlerResult;
use libloading::Symbol;
use ppd_shared::{
    opts::{ServiceAuthMode, ServiceConfig, ServiceType},
    plugin::{HasDependecies, Plugin},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct Service<'a> {
    ty: &'a ServiceType,
    port: &'a u16,
    modes: &'a [ServiceAuthMode],
    auto_install: &'a bool
}

impl<'a> Service<'a> {
    /// start a rest or grpc service
    pub fn start(&self, config: ServiceConfig, token: CancellationToken) -> HandlerResult<()> {
        let filename = self.output()?;

        let cfg_ptr = Arc::new(config);
        let cfg_raw = Arc::into_raw(cfg_ptr);

        let token = Box::new(token);
        let token_raw = Box::into_raw(token);

        let lib = self.load(filename)?;
        let start: Symbol<unsafe extern "C" fn(*const ServiceConfig, *mut CancellationToken)> = unsafe {
            lib.get(b"start_svc")
                .expect("unable to load start_server Symbol")
        };

        unsafe { start(cfg_raw, token_raw) };

        Ok(())
    }

    pub fn ty(&self) -> &ServiceType {
        &self.ty
    }

    pub fn port(&self) -> &u16 {
        &self.port
    }

    pub fn modes(&self) -> &[ServiceAuthMode] {
        self.modes
    }

    pub fn auto_install(&self) -> bool {
        *self.auto_install
    }
}

impl<'a> From<&'a ServiceConfig> for Service<'a> {
    fn from(value: &'a ServiceConfig) -> Self {
        Self {
            ty: &value.ty,
            port: &value.base.port,
            modes: &value.auth.modes,
            auto_install: &value.auto_install
        }
    }
}

impl<'a> Plugin for Service<'a> {
    fn package_name(&self) -> &'static str {
        match &self.ty {
            ServiceType::Rest => "ppd_rest",
            ServiceType::Grpc => "ppd_grpc",
        }
    }
}

impl<'a> HasDependecies for Service<'a> {
    fn dependecies(&self) -> Vec<Box<dyn Plugin>> {
        let routers: Vec<Box<dyn Plugin>> = self
            .modes
            .iter()
            .map(|mode| {
                Box::new(ServiceRouter {
                    svc_type: *self.ty,
                    auth_mode: mode.clone(),
                }) as Box<dyn Plugin>
            })
            .collect();

        routers
    }
}

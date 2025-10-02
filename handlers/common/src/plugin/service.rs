use std::{pin::Pin, sync::Arc};

use super::router::ServiceRouter;
use crate::{HandlerResult, errors::HandlerError};
use libloading::Symbol;
use ppd_bk::RBatis;
use ppd_shared::{
    opts::{ServiceAuthMode, ServiceConfig, ServiceType},
    plugin::{HasDependecies, Plugin},
};
use tokio_util::sync::CancellationToken;

pub type CallResult = Pin<Box<dyn Future<Output = Result<(), HandlerError>>>>;

#[derive(Debug)]
pub struct Service<'a> {
    ty: &'a ServiceType,
    port: &'a u16,
    modes: &'a [ServiceAuthMode],
    auto_install: &'a bool,
}

impl<'a> Service<'a> {
    /// start a rest or grpc service
    pub async fn start(
        &self,
        config: ServiceConfig,
        db: Arc<RBatis>,
        token: CancellationToken,
    ) -> HandlerResult<()> {
        let filename = self.output()?;
        let config = Arc::new(config);

        let lib = self.load(filename)?;
        let start_service: Symbol<fn(Arc<ServiceConfig>, Arc<RBatis>, CancellationToken)> = unsafe {
            lib.get(b"start_svc")
                .expect("unable to load start_server Symbol")
        };

        start_service(config, db, token);
        Ok(())
    }

    /// preload service and its dependencies
    pub fn init(&self) -> HandlerResult<()> {
        let auto_install = self.auto_install();
        self.preload_deps(auto_install)?;
        self.preload(auto_install)?;

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
            auto_install: &value.auto_install,
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

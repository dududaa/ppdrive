use std::{net::TcpStream, sync::Arc};

use super::router::ServiceRouter;
use crate::HandlerResult;
use ppd_bk::RBatis;
use ppd_shared::{
    opts::{ServiceAuthMode, ServiceConfig, ServiceType},
    plugin::{Module, Plugin},
};
use tokio_util::sync::CancellationToken;

pub type ServiceFn = fn(Arc<ServiceConfig>, Arc<RBatis>, CancellationToken);

#[derive(Debug)]
pub struct Service<'a> {
    ty: &'a ServiceType,
    port: &'a u16,
    modes: &'a [ServiceAuthMode],
    auto_install: &'a bool,
    reload_deps: &'a bool,
}

impl<'a> Service<'a> {
    /// start a rest or grpc service
    pub async fn start(
        &self,
        config: ServiceConfig,
        db: Arc<RBatis>,
        token: CancellationToken,
    ) -> HandlerResult<()> {
        let filename = self.output_name()?;
        let config = Arc::new(config);

        let lib = self.load(filename)?;
        unsafe {
            match lib.get::<ServiceFn>(b"start_svc") {
                Ok(start_service) => start_service(config, db, token),
                Err(err) => tracing::error!("unable to load start_server Symbol: {err}")
            }
        };

        Ok(())
    }

    /// preload service and its dependencies
    pub fn init(&self) -> HandlerResult<()> {
        let auto_install = self.auto_install();
        let reload = self.reload_deps();

        self.preload_deps(auto_install, reload)?;
        self.preload(auto_install, reload)?;

        Ok(())
    }

    pub fn connect(&self) -> HandlerResult<()> {
        TcpStream::connect(&self.addr())?;
        Ok(())
    }

    pub fn ty(&self) -> &ServiceType {
        self.ty
    }

    pub fn port(&self) -> &u16 {
        self.port
    }

    pub fn modes(&self) -> &[ServiceAuthMode] {
        self.modes
    }

    pub fn auto_install(&self) -> bool {
        *self.auto_install
    }

    pub fn reload_deps(&self) -> bool {
        *self.reload_deps
    }

    fn addr(&self) -> String {
        format!("0.0.0.0:{}", self.port())
    }
}

impl<'a> From<&'a ServiceConfig> for Service<'a> {
    fn from(value: &'a ServiceConfig) -> Self {
        Self {
            ty: &value.ty,
            port: &value.base.port,
            modes: &value.auth.modes,
            auto_install: &value.auto_install,
            reload_deps: &value.reload_deps,
        }
    }
}

impl<'a> Plugin for Service<'a> {
    fn package_name(&self) -> &'static str {
        match &self.ty {
            ServiceType::Rest => "ppd-rest",
            ServiceType::Grpc => "ppd-grpc",
        }
    }
}

impl<'a> Module for Service<'a> {
    fn dependecies(&self) -> Vec<Box<dyn Plugin>> {
        let routers: Vec<Box<dyn Plugin>> = self
            .modes
            .iter()
            .map(|mode| {
                Box::new(ServiceRouter {
                    svc_type: *self.ty,
                    auth_mode: *mode,
                }) as Box<dyn Plugin>
            })
            .collect();

        routers
    }
}

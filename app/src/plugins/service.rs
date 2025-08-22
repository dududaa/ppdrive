use std::{path::PathBuf, process::Command};

use clap::ValueEnum;
use libloading::{Library, Symbol};
use ppd_shared::tools::root_dir;

use crate::{errors::AppResult, plugins::Plugin};

#[derive(Default)]
pub struct Service {
    ty: ServiceType,
    auth_mode: Option<ServiceAuthMode>,
    port: Option<u16>
}

impl Service {
    pub fn start(&self) -> AppResult<()> {
        let filename = self.filename()?;

        #[cfg(debug_assertions)]
        if let Err(err) = std::fs::remove_file(&filename) {
            eprintln!("cannot remove previous plugin: {err}")
        }

        if !filename.is_file() {
            // confirm before installation
            println!("You currently don't have {} plugin installed. Do you want to install it? (y/n)", self.plugin_name());
            let mut ans = String::new();

            std::io::stdin().read_line(&mut ans)?;
            let ans = ans.trim().to_lowercase();
            
            if &ans == "y" {
                println!("installing {}...", self.plugin_name());
                self.install()?;
            } else {
                return Ok(());
            }
        }

        let lib = unsafe { Library::new(&filename)? };
        let start: Symbol<unsafe extern "C" fn (u16)> = unsafe {
            lib.get(b"start_server")?
        };

        let port = self.port.unwrap_or(5000);
        unsafe { start(port); }

        println!("server started at port: {port}");

        Ok(())
    }

    fn plugin_name(&self) -> String {
        let mut n = format!("{:?}", self.ty);
        if let Some(mode) = self.auth_mode {
            n.push_str(&format!("-{mode:?}"));
        }

        n
    }
}

impl Plugin for Service {
    fn filename(&self) -> AppResult<PathBuf> {
        let mut n = self.plugin_name();
        n.push_str(Self::ext());

        let p = root_dir()?.join(n.to_lowercase());

        Ok(p)
    }

    fn package_name(&self) -> &'static str {
        match &self.ty {
            ServiceType::Rest => "ppd_rest",
            ServiceType::Grpc => "ppd_grpc",
        }
    }

    #[cfg(debug_assertions)]
    fn install_local(&self) -> AppResult<()> {
        let mut args = vec!["build", "-p", self.package_name()];
        if let Some(mode) = &self.auth_mode {
            let mode = match mode {
                ServiceAuthMode::Client => "client-auth",
                ServiceAuthMode::User => "user-auth",
                ServiceAuthMode::Zero => "zero-auth",
            };

            args.append(&mut vec!["--features", mode, "--no-default-features"]);
        }

        let mut child = Command::new("cargo").args(args).spawn()?;
        child.wait()?;

        let release_path = self.release_path()?;
        let output_path = self.filename()?;

        std::fs::rename(release_path, output_path)?;

        Ok(())
    }
}

pub struct ServiceBuilder {
    inner: Service,
}

impl ServiceBuilder {
    pub fn new(ty: ServiceType) -> Self {
        let inner = Service {
            ty,
            ..Default::default()
        };
        Self { inner }
    }

    pub fn auth_mode(mut self, auth_mode: Option<ServiceAuthMode>) -> Self {
        self.inner.auth_mode = auth_mode;
        self
    }

    pub fn port(mut self, port: Option<u16>) -> Self {
        self.inner.port = port;
        self
    }

    pub fn build(self) -> Service {
        self.inner
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Default, Debug)]
pub enum ServiceType {
    #[default]
    Rest,
    Grpc,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum ServiceAuthMode {
    Client,
    User,
    Zero,
}

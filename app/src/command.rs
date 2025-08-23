use clap::{Parser, Subcommand};

use crate::{
    errors::AppResult,
    manager::{ServiceInfo, ServiceManager},
};
use ppd_shared::{
    config::AppConfig,
    plugins::service::{ServiceAuthMode, ServiceBuilder, ServiceType},
};

use tokio_util::sync::CancellationToken;

/// A free and open-source cloud storage service.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    /// server's authentication mode
    #[arg(long, value_enum)]
    mode: Option<Vec<ServiceAuthMode>>,

    /// the port to run server on
    #[arg(long, short)]
    port: Option<u16>,
}

impl Cli {
    pub async fn run(&self) -> AppResult<()> {
        let mut config = AppConfig::load().await?;
        if let Some(modes) = &self.mode {
            println!("updating configuration...");
            config.set_auth_modes(modes).await?;
            println!("configuration updated...");
        }

        match self.command {
            CliCommand::Start { ty } => {
                let mut builder = ServiceBuilder::new(ty.clone());
                if let Some(port) = self.port {
                    builder = builder.port(port);
                }

                let svc = builder.build();
                let port = svc.port().clone();

                let token = CancellationToken::new();
                let token_clone = token.clone();
                tokio::spawn(async move {
                    tokio::select! {
                        res = svc.start() => {
                            if let Err(err) = res {
                                panic!("unable to start service: {err}")
                            }
                        }
                        _ = token_clone.cancelled() => {
                            print!("service closed successfully")
                        }
                    }
                });

                // add task to service task to manager
                let info = ServiceInfo { ty, token };
                ServiceManager::add_svc(info, &config).await?;

                println!("server started at port {}", port);
            }
            CliCommand::Stop { ty } => {
                ServiceManager::cancel_svc(ty, &config).await?;
            }
            CliCommand::Manager => {
                let mut manager = ServiceManager::default();
                tokio::spawn(async move {
                    if let Err(err) = manager.start(&config).await {
                        panic!("{err}")
                    }
                });
            }
            _ => unimplemented!("this command is not supported"),
        }

        Ok(())
    }
}

#[derive(Subcommand)]
enum CliCommand {
    /// start or restart a server
    Start {
        #[arg(value_enum)]
        ty: ServiceType,
    },

    /// stop a running server
    Stop {
        #[arg(value_enum)]
        ty: ServiceType,
    },

    /// install a module
    Install,

    /// a command for starting service manager
    Manager,
}

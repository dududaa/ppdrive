use clap::{Parser, Subcommand};

use crate::errors::AppResult;
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
        if let Some(modes) = &self.mode {
            println!("updating configuration...");
            let mut config = AppConfig::load().await?;
            config.set_auth_modes(modes).await?;
            println!("configuration updated...");
        }

        match self.command {
            CliCommand::Start { ty } => {
                let svc = ServiceBuilder::new(ty).port(self.port).build();
                let token = CancellationToken::new();

                let port = svc.start().await?;
                
                println!("server started at port {port}");
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
    Stop,

    /// install a module
    Install,
}

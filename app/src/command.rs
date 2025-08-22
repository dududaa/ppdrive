use clap::{Parser, Subcommand};

use crate::{
    errors::AppResult,
    plugins::service::{ServiceAuthMode, ServiceBuilder, ServiceType},
};

/// A free and open-source cloud storage service.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    /// server's authentication mode
    #[arg(long, value_enum)]
    mode: Option<ServiceAuthMode>,

    /// the port to run server on
    #[arg(long, short)]
    port: Option<u16>,
}

impl Cli {
    pub fn run(&self) -> AppResult<()> {
        match self.command {
            CliCommand::Start { ty } => {
                let svc = ServiceBuilder::new(ty)
                    .auth_mode(self.mode)
                    .port(self.port)
                    .build();
                
                svc.start()?;
            }
            _ => unimplemented!(),
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

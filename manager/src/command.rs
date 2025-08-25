use clap::{Parser, Subcommand, ValueEnum};
use tracing::instrument;

use crate::{errors::AppResult, manage::ServiceManager};
use ppd_shared::plugins::service::{
    ServiceAuthConfig, ServiceBaseConfig, ServiceConfig, ServiceType,
};

/// A free and open-source cloud storage service.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    /// the port to run service manager on
    #[arg(long, short)]
    port: Option<u16>,
}

impl Cli {
    #[instrument]
    pub async fn run(self) -> AppResult<()> {
        let port = self.port.clone();

        match self.command {
            CliCommand::Start {
                select,
                base_config,
                auth,
            } => match select {
                StartOptions::Manager => {
                    let mut manager = ServiceManager::default();
                    manager.start(port)?;
                }
                _ => {
                    let config = ServiceConfig::default()
                    .auth(auth)
                    .base(base_config)
                    .service_type(select.into());

                    tracing::info!("adding service to service manager...");
                    ServiceManager::add(config, port)?;
                }
            },
            CliCommand::Stop { id } => {
                ServiceManager::cancel(id, port)?;
            }
            CliCommand::List => {
                ServiceManager::list(port)?;
            }
            _ => unimplemented!("this command is not supported"),
        }

        Ok(())
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// start a service or service manager
    Start {
        #[arg(value_enum)]
        select: StartOptions,

        #[command(flatten)]
        base_config: Option<ServiceBaseConfig>,

        #[command(flatten)]
        auth: Option<ServiceAuthConfig>,
    },

    /// stop a running service
    Stop { id: u8 },

    /// install a module
    Install,

    /// list services running in service manager
    List
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum StartOptions {
    Manager,
    Rest,
    Grpc,
}

impl From<StartOptions> for ServiceType {
    fn from(value: StartOptions) -> Self {
        match value {
            StartOptions::Grpc => ServiceType::Grpc,
            StartOptions::Rest => ServiceType::Rest,
            _ => unreachable!("service unknown"),
        }
    }
}

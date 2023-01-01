use crate::{errors::AppResult, manage::ServiceManager};
use clap::{Parser, Subcommand, ValueEnum};
use ppd_shared::opts::{ServiceAuthConfig, ServiceBaseConfig, ServiceConfig, ServiceType};
use tracing_appender::non_blocking::WorkerGuard;

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
    pub async fn run(self, guard: WorkerGuard) -> AppResult<()> {
        let port = self.port.clone().unwrap_or(5025);

        match self.command {
            CliCommand::Start {
                select,
                base_config,
                auth,
            } => match select {
                StartOptions::Manager => {
                    let manager = ServiceManager::default();
                    manager.start(port, guard).await?;
                }
                _ => {
                    let config = ServiceConfig {
                        ty: select.into(),
                        base: base_config,
                        auth,
                    };

                    tracing::info!("adding service to service manager...");
                    ServiceManager::add(config, port).await?;
                }
            },
            CliCommand::Stop { id } => {
                ServiceManager::cancel(id, port).await?;
            }
            CliCommand::List => {
                ServiceManager::list(port).await?;
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
        base_config: ServiceBaseConfig,

        #[command(flatten)]
        auth: ServiceAuthConfig,
    },

    /// stop a running service
    Stop { id: u8 },

    /// install a module
    Install,

    /// list services running in service manager
    List,
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

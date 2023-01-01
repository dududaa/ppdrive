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
            CliCommand::Start => {
                let manager = ServiceManager::default();
                manager.start(port, guard).await?;
            }
            CliCommand::Run {
                svc,
                base_config,
                auth_config,
                yes_auto_install: auto_install,
            } => {
                let config = ServiceConfig {
                    ty: svc,
                    base: base_config,
                    auth: auth_config,
                    auto_install,
                };

                ServiceManager::add(config, port).await?;
            }
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
    /// start ppdrive service manager
    Start,

    /// run a ppdrive service
    Run {
        svc: ServiceType,

        #[command(flatten)]
        base_config: ServiceBaseConfig,

        #[command(flatten)]
        auth_config: ServiceAuthConfig,

        /// automatically install missing plugins and dependencies
        #[arg(default_value_t = false, short)]
        yes_auto_install: bool,
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

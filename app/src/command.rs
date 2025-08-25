use clap::{Parser, Subcommand};
use tracing::instrument;

use crate::{errors::AppResult, manager::ServiceManager};
use ppd_shared::plugins::service::{ServiceAuthConfig, ServiceAuthMode, ServiceBaseConfig, ServiceConfig};

/// A free and open-source cloud storage service.
#[derive(Parser, Debug)]
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
    #[instrument]
    pub async fn run(self) -> AppResult<()> {
        match self.command {
            CliCommand::Start { base_config, auth } => {
                let config = ServiceConfig::default().auth(auth).base(base_config);
                tracing::info!("adding service to service manager...");
                ServiceManager::add_svc(config, None)?;
            }
            CliCommand::Stop { id } => {
                ServiceManager::cancel_svc(id, None)?;
            }
            CliCommand::Manager { port } => {
                let mut manager = ServiceManager::default();
                manager.start(port)?;
            }
            _ => unimplemented!("this command is not supported"),
        }

        Ok(())
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// start or restart a server
    Start {
        #[command(flatten)]
        base_config: Option<ServiceBaseConfig>,

        #[command(flatten)]
        auth: Option<ServiceAuthConfig>,
    },

    /// stop a running server
    Stop {
        #[arg(long)]
        id: u8,
    },

    /// install a module
    Install,

    /// a command for starting service manager
    Manager {
        #[arg(long, short)]
        port: Option<u16>,
    },
}

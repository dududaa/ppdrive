use std::process::Command;

use crate::{errors::AppResult, manage::PPDrive};
use clap::{Parser, Subcommand, ValueEnum};
use ppd_shared::opts::{ServiceAuthConfig, ServiceBaseConfig, ServiceConfig, ServiceType};

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
    pub async fn run(self) -> AppResult<()> {
        let port = self.port.clone().unwrap_or(5025);

        match self.command {
            CliCommand::Start => {
                let prog = if cfg!(debug_assertions) {
                    "cargo"
                } else {
                    "manager"
                };
                let mut cmd = Command::new(prog);

                if cfg!(debug_assertions) {
                    cmd.args(["run", "--bin", "manager"]);
                }

                cmd.arg(port.to_string());
                cmd.spawn()?;
            }

            CliCommand::Status => {
                PPDrive::check_status(port).await?;
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

                PPDrive::add(config, port).await?;
            }
            CliCommand::Stop { id } => match id {
                Some(id) => PPDrive::cancel(id, port).await?,
                None => PPDrive::stop(port).await?,
            },
            CliCommand::List => {
                PPDrive::list(port).await?;
            }
            CliCommand::CreateClient {
                svc_id,
                client_name,
            } => {
                PPDrive::create_client(port, svc_id, client_name).await?;
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

    /// check whether ppdrive instance is running (on the specified port).
    Status,

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

    /// stop ppdrive or a running service.
    /// if id is provided, this will try to stop a running service else, the manager will stopped.
    Stop { id: Option<u8> },

    /// create a new client for the specified service
    CreateClient { svc_id: u8, client_name: String },

    /// list services running in service manager
    List,

    /// install a module
    Install,
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

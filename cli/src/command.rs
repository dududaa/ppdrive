use std::{path::Path, process::Command, thread::sleep, time::Duration};

use crate::{errors::AppResult, imp::PPDrive};
use clap::{Parser, Subcommand, ValueEnum};
use ppd_shared::{
    opts::internal::{ServiceAuthConfig, ServiceBaseConfig, ServiceConfig, ServiceType},
    tools::root_dir,
};

/// PPDRIVE is a free, open-source cloud storage service built with Rust for speed, security, and reliability.
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
    pub fn run(self) -> AppResult<()> {
        let port = self.port.unwrap_or(5025);

        match self.command {
            CliCommand::Start => {
                tracing::info!("start ppdrive manager...");
                start_manager::<String>(port, None)?;

                tracing::info!("checking ppdrive status...");
                sleep(Duration::from_secs(2));

                match PPDrive::check_status(port) {
                    Ok(_) => tracing::info!("ppdrive started successfully."),
                    Err(err) => tracing::info!(
                        "fail to connect to ppdrive manager {err}.\nPlease check logs for more info."
                    ),
                }
            }

            CliCommand::Status => {
                PPDrive::check_status(port)?;
            }

            CliCommand::Launch {
                svc,
                base_config,
                auth_config,
                yes_auto_install: auto_install,
                remove_deps: reload,
            } => {
                let config = ServiceConfig {
                    ty: svc,
                    base: base_config,
                    auth: auth_config,
                    auto_install,
                    reload_deps: reload,
                };

                PPDrive::add(config, port)?;
            }
            CliCommand::Stop { id } => match id {
                Some(id) => PPDrive::cancel(id, port)?,
                None => PPDrive::stop(port)?,
            },
            CliCommand::List => {
                PPDrive::list(port)?;
            }
            CliCommand::Client { command } => match command {
                ClientCommand::Create {
                    service_id: svc_id,
                    client_name,
                    max_bucket_size,
                } => {
                    PPDrive::create_client(port, svc_id, client_name, max_bucket_size)?;
                }
                ClientCommand::Refresh {
                    service_id: svc_id,
                    client_id: client_key,
                } => {
                    PPDrive::refresh_client_token(port, svc_id, client_key)?;
                }
                ClientCommand::List { service_id } => {
                    PPDrive::get_client_list(port, service_id)?;
                }
            },
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

    /// launch a ppdrive service
    Launch {
        svc: ServiceType,

        #[command(flatten)]
        base_config: ServiceBaseConfig,

        #[command(flatten)]
        auth_config: ServiceAuthConfig,

        /// automatically install missing plugins and dependencies
        #[arg(default_value_t = false, short)]
        yes_auto_install: bool,

        /// removes all dependencies for the service you wish to run. Ideally, you should
        /// use this option with `-y | yes-auto-install` set to `true`. For example:
        /// `ppdrive launch rest -ry`
        #[arg(default_value_t = false, short)]
        remove_deps: bool,
    },

    /// stop ppdrive or a running service.
    /// if id is provided, this will try to stop a running service else, the manager will stopped.
    Stop { id: Option<u8> },

    /// create a new client for the specified service
    // CreateClient { svc_id: u8, client_name: String },
    Client {
        #[command(subcommand)]
        command: ClientCommand,
    },
    /// list services running in service manager
    List,

    /// install a module
    Install,
}

#[derive(Subcommand, Debug)]
enum ClientCommand {
    /// create a new client and receive the client token.
    Create {
        #[arg(long("svc-id"))]
        service_id: u8,

        #[arg(long("name"))]
        client_name: String,

        #[arg(long)]
        /// total maximum size of buckets that this client can create
        max_bucket_size: Option<f64>,
    },

    /// refresh token for a given client.
    Refresh {
        #[arg(long("svc-id"))]
        service_id: u8,

        #[arg(long("client-id"))]
        client_id: String,
    },

    List {
        #[arg(long("svc-id"))]
        service_id: u8,
    },
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

/// start the manager by running appropriate command based on environments. `current_dir`
/// is the directory from which we run the command.
pub fn start_manager<P>(port: u16, current_dir: Option<P>) -> AppResult<()>
where
    P: AsRef<Path>,
{
    let prog = if cfg!(debug_assertions) {
        "cargo".into()
    } else {
        root_dir()?.join("manager")
    };

    let mut cmd = Command::new(prog);

    if let Some(cd) = current_dir {
        cmd.current_dir(cd);
    }

    if cfg!(debug_assertions) {
        cmd.args(["run", "--bin", "manager"]);
    }

    cmd.arg(port.to_string());
    cmd.spawn()?;

    Ok(())
}

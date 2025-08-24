use clap::{Parser, Subcommand};
use tracing::{Instrument, info_span, instrument};

use crate::{
    errors::AppResult,
    manager::{ServiceInfo, ServiceManager},
    state::SyncState,
};
use ppd_shared::plugins::service::{ServiceAuthMode, ServiceBuilder, ServiceType};

use tokio_util::sync::CancellationToken;

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
    pub async fn run(&self, state: SyncState) -> AppResult<()> {
        if let Some(modes) = &self.mode {
            tracing::info!("updating configuration...");
            let mut state = state.lock().await;
            state.update_auth_modes(modes).await?;
            tracing::info!("configuration updated...");
        }

        match self.command {
            CliCommand::Start { ty } => {
                let mut builder = ServiceBuilder::new(ty.clone());
                if let Some(port) = self.port {
                    builder = builder.port(port);
                }

                let svc = builder.build();

                let token = CancellationToken::new();
                let token_clone = token.clone();

                // add task to service task to manager
                tracing::info!("adding service to service manager...");
                let info = ServiceInfo { ty, token };
                ServiceManager::add_svc(info, state).await?;
                
                tokio::spawn(
                    async move {
                        tracing::info!("preparing to start service...");
                        tokio::select! {
                            res = svc.start() => {
                                if let Err(err) = res {
                                    tracing::error!("unable to start service: {err}")
                                }
                            }
                            _ = token_clone.cancelled() => {
                                tracing::info!("service closed successfully")
                            }
                        }
                    }
                    .instrument(info_span!("start_server")),
                );
            }
            CliCommand::Stop { ty } => {
                ServiceManager::cancel_svc(ty, state).await?;
            }
            CliCommand::Manager => {
                let mut manager = ServiceManager::default();
                let state = state;

                tokio::spawn(
                    async move {
                        match manager.start(state).await {
                            Ok(_) => tracing::info!("manager successfully..."),
                            Err(err) => tracing::error!(?err, "error occured"),
                        }
                    }
                    .instrument(info_span!("start_manager")),
                );
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

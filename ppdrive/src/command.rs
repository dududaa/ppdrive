use clap::{Parser, Subcommand};
use shared::client::{create_client, regenerate_token};
use shared::config::AppConfig;
use shared::create_pool;
use shared::secrets::AppSecrets;
use std::process::Command;

/// PPDRIVE is a free, open-source object storage service built with Rust for speed, security,
/// and reliability.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

impl Cli {
    pub async fn execute(&self) -> Result<(), anyhow::Error> {
        let config = AppConfig::read().await?;
        let pool = create_pool(&config.database_url).await?;
        let secret = AppSecrets::read().await?;

        match &self.command {
            CliCommand::Client { command } => match command {
                ClientCommand::Create {
                    client_name,
                    max_bucket_size,
                } => {
                    let client =
                        create_client(&pool, &secret, client_name, *max_bucket_size).await?;

                    println!("client created successfully!");
                    println!("client_id: {}", client.id());
                    println!("token: {}", client.token());
                }
                ClientCommand::Refresh { client_id } => {
                    let token = regenerate_token(&pool, &secret, client_id).await?;
                    println!("client token refreshed successfully!");
                    println!("token: {}", token);
                }
                _ => {}
            }

            CliCommand::Serve => {
                if cfg!(debug_assertions) {
                    Command::new("cargo")
                        .args(["run", "-p", "server"])
                        .status()?;
                } else {
                    Command::new("./server").status()?;
                }
            }

            CliCommand::Configure => {
                Command::new("nano")
                    .arg("ppd_config.toml")
                    .status()?;
            }
        }

        Ok(())
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    Serve,
    Configure,
    /// create a new client
    Client {
        #[command(subcommand)]
        command: ClientCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ClientCommand {
    /// create a new client and receive the client token.
    Create {
        /// Arbitrary name to remember the client. Use a name that describes the client application(s), e.g MyGoodness App
        #[arg(long("name"))]
        client_name: String,

        #[arg(long)]
        /// Total maximum size of buckets that this client can create.
        max_bucket_size: Option<f64>,
    },

    /// refresh token for a given client.
    Refresh {
        #[arg(long("id"))]
        client_id: String,
    },

    List,
}

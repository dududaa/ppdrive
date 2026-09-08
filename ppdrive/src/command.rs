use crate::subs::{BucketCommand, ClientCommand};
use clap::{Parser, Subcommand};
use shared::db::client::{create_client, regenerate_token};
use shared::db::bucket;
use shared::config::AppConfig;
use shared::db::Database;
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

/// Parse CLI arguments and execute the corresponding subcommand.
impl Cli {
    pub async fn execute(&self) -> Result<(), anyhow::Error> {
        let config = AppConfig::read().await?;
        let pool = Database::new(&config.database_url).await?;
        AppSecrets::init().await?;
        let secret = AppSecrets::read().await?;

        match &self.command {
            CliCommand::Client { command } => match command {
                ClientCommand::Create { client_name } => {
                    let client = create_client(&pool, &secret, client_name).await?;

                    println!("Client created successfully!");
                    println!("Client ID: {}", client.id());
                    println!("Client Token: {}", client.token());
                }
                ClientCommand::Refresh { client_id } => {
                    let token = regenerate_token(&pool, &secret, client_id).await?;
                    println!("Client token refreshed successfully!");
                    println!("Client Token: {}", token);
                }
                _ => {}
            },

            CliCommand::Bucket { command } => match command {
                BucketCommand::Create(args) => {
                    let owner_id = shared::db::client::get_id(&args.owner_id, &pool).await?;
                    let data = args.clone().into_data(owner_id);
                    let id = bucket::create(&data, &pool).await?;

                    println!("Bucket created successfully!");
                    println!("Bucket ID: {id}");
                }
            },

            CliCommand::Serve { port } => {
                if cfg!(debug_assertions) {
                    Command::new("cargo")
                        .args(["run", "-p", "server"])
                        .arg(port.to_string())
                        .status()?;
                } else {
                    Command::new("./server").arg(port.to_string()).status()?;
                }
            }

            CliCommand::Configure => {
                let editor = std::env::var("VISUAL")
                    .or_else(|_| std::env::var("EDITOR"))
                    .unwrap_or_else(|_| "nano".to_string());
                Command::new(&editor).arg("ppd_config.toml").status()?;
            }
        }

        Ok(())
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    Serve {
        #[arg(long = "port")]
        port: u16,
    },
    Configure,
    /// create a new client
    Client {
        #[command(subcommand)]
        command: ClientCommand,
    },
    Bucket {
        #[command(subcommand)]
        command: BucketCommand,
    },
}

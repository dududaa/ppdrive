use clap::{Parser, Subcommand};
use shared::client::{create_client, regenerate_token};
use shared::create_pool;
use shared::secrets::AppSecrets;

/// PPDRIVE is a free, open-source object storage service built with Rust for speed, security,
/// and reliability.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    #[clap(short, long)]
    database_url: String,
}

impl Cli {
    pub async fn execute(&self) -> Result<(), anyhow::Error> {
        let pool = create_pool(&self.database_url).await?;
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
            },
        }

        Ok(())
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// create a new client for the specified service
    Client {
        #[command(subcommand)]
        command: ClientCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ClientCommand {
    /// create a new client and receive the client token.
    Create {
        #[arg(long("name"))]
        client_name: String,

        #[arg(long)]
        /// total maximum size of buckets that this client can create
        max_bucket_size: Option<f64>,
    },

    /// refresh token for a given client.
    Refresh {
        #[arg(long("id"))]
        client_id: String,
    },

    List,
}

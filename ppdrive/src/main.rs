use clap::{Parser, Subcommand};


fn main() {
    println!("Hello, world!");
}

/// PPDRIVE is a free, open-source object storage service built with Rust for speed, security,
/// and reliability.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,
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
        #[arg(long("client-id"))]
        client_id: String,
    },

    List,
}

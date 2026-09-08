//! PPDRIVE command-line interface.
//!
//! A CLI tool for managing PPDRIVE resources: creating clients, provisioning buckets,
//! launching the storage server, and editing the configuration file.

use crate::command::Cli;
use clap::Parser;

mod command;
mod subs;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    cli.execute().await?;

    Ok(())
}

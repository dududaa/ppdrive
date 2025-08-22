use clap::Parser;

use crate::{command::Cli, errors::AppResult};

mod command;
mod errors;

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();
    cli.run().await?;

    Ok(())
}

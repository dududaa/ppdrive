use crate::command::Cli;
use clap::Parser;

mod command;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    cli.execute().await?;

    Ok(())
}

use clap::Parser;
use crate::{command::Cli, errors::AppResult, state::State};

mod command;
mod errors;
mod manager;

mod state;

#[tokio::main]
async fn main() -> AppResult<()> {
    
    let cli = Cli::parse();
    let state = State::create().await?;
    if let Err(err)  = cli.run(state.get()).await {
        tracing::error!("{err}")
    }

    Ok(())
}

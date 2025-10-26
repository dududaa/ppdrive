use crate::{command::Cli, errors::AppResult};
use clap::Parser;
use ppd_shared::start_logger;

mod command;
mod errors;
mod imp;

#[cfg(test)]
mod tests;

fn main() -> AppResult<()> {
    let cli = Cli::parse();
    let _guard = start_logger("ppdrive=debug,ppd_shared=debug")?;

    if let Err(err) = cli.run() {
        tracing::error!("{err}")
    }

    Ok(())
}
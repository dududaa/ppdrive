use clap::Parser;

use crate::{command::Cli, errors::AppResult};

mod command;
mod errors;

fn main() -> AppResult<()> {
    let cli = Cli::parse();
    cli.run()?;

    Ok(())
}

use std::fs::OpenOptions;

use crate::{command::Cli, errors::AppResult};
use clap::Parser;
use tracing_appender::non_blocking as non_blocking_logger;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

mod command;
mod errors;
mod manage;

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();
    let _guard = start_logger()?;

    if let Err(err) = cli.run().await {
        tracing::error!("{err}")
    }

    Ok(())
}

fn start_logger() -> AppResult<tracing_appender::non_blocking::WorkerGuard> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("ppd.log")?;
    
    let (writer, guard) = non_blocking_logger(log_file);

    if let Err(err) = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppdrive=debug,ppd_shared=debug,ppd_rest=debug".into()),
        )
        .with(fmt::layer().with_ansi(false).pretty().with_writer(writer))
        .with(fmt::layer().without_time().pretty())
        .try_init()
    {
        println!("cannot start logger: {err}")
    }

    Ok(guard)
}

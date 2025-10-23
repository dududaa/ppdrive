use std::fs::OpenOptions;

use crate::errors::Error;
pub mod errors;
pub mod tools;
pub mod plugin;
pub mod opts;

pub type AppResult<T> = Result<T, Error>;

#[cfg(feature = "logger")]
pub fn start_logger(log_filter: &str) -> AppResult<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::non_blocking;
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("ppd.log")?;
    
    let (writer, guard) = non_blocking(log_file);

    if let Err(err) = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(fmt::layer().with_ansi(false).pretty().with_writer(writer))
        .with(fmt::layer().without_time().pretty())
        .try_init()
    {
        println!("cannot start logger: {err}")
    }

    Ok(guard)
}
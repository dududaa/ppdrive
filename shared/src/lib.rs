use std::fs::OpenOptions;

use crate::errors::Error;
pub mod errors;
pub mod opts;
pub mod plugin;
pub mod tools;

pub type AppResult<T> = Result<T, Error>;

#[cfg(feature = "logger")]
pub fn start_logger(log_filter: &str) -> AppResult<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::non_blocking;
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("ppd.log")?;

    #[cfg(debug_assertions)]
    let stdout_layer = fmt::layer().without_time().pretty();

    #[cfg(not(debug_assertions))]
    let stdout_layer = fmt::layer()
        .with_target(false)
        .without_time()
        .with_file(false)
        .with_line_number(false)
        .compact();

    let (writer, guard) = non_blocking(log_file);

    if let Err(err) = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| log_filter.into()))
        .with(fmt::layer().with_ansi(false).pretty().with_writer(writer))
        .with(stdout_layer)
        .try_init()
    {
        println!("cannot start logger: {err}")
    }

    Ok(guard)
}

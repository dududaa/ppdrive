use crate::errors::Error;
pub mod errors;
pub mod opts;
pub mod plugin;
pub mod tools;

#[cfg(feature = "api")]
pub mod api;

pub type AppResult<T> = Result<T, Error>;

#[cfg(feature = "logger")]
type LoggerGuard = tracing_appender::non_blocking::WorkerGuard;

#[cfg(feature = "logger")]
pub fn start_logger(log_filter: &str) -> AppResult<LoggerGuard> {
    use std::fs::OpenOptions;
    use tracing_appender::non_blocking;
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    use crate::tools::root_dir;

    let filepath = root_dir()?.join("ppd.log");
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filepath)?;

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

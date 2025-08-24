use std::sync::Arc;
use tokio::sync::Mutex;

use ppd_shared::{config::AppConfig, plugins::service::ServiceAuthMode};
use tracing_appender::non_blocking as non_blocking_logger;

use crate::errors::AppResult;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub type SyncState = Arc<Mutex<State>>;

#[derive(Debug)]
pub struct AppState(SyncState);
impl AppState {
    pub fn get(&self) -> SyncState {
        self.0.clone()
    }
}

#[derive(Debug)]
pub struct State {
    config: AppConfig,
    logger: (
        tracing_appender::non_blocking::NonBlocking,
        tracing_appender::non_blocking::WorkerGuard,
    ),
}

impl State {
    pub async fn create() -> AppResult<AppState> {
        let log_file = std::fs::File::create("ppd.log")?;

        let logger = non_blocking_logger(log_file);
        let config = AppConfig::load().await?;

        let ss = State { logger, config };

        if let Err(err) = tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "ppdrive=debug,ppd_shared=debug,ppd_rest=debug".into()))
            .with(
                fmt::layer()
                    .with_ansi(false)
                    .pretty()
                    .with_writer(ss.logger.0.clone()),
            )
            .with(fmt::layer().pretty())
            .try_init()
        {
            println!("cannot start logger: {err}")
        }

        tracing::info!("logger started...");
        let state = Arc::new(Mutex::new(ss));
        Ok(AppState(state))
    }

    pub async fn update_auth_modes(&mut self, modes: &[ServiceAuthMode]) -> AppResult<()> {
        self.config.set_auth_modes(modes).await?;
        Ok(())
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }
}

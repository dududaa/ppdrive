use std::env::set_var;

use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue};
use axum::{
    extract::MatchedPath,
    http::Request,
    routing::{get, IntoMakeService},
    Router,
};
use handlers::plugin::router::ServiceRouter;
use ppd_shared::opts::{ServiceAuthMode, ServiceConfig, ServiceType};
use tower_http::cors::{AllowOrigin, Any};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::errors::ServerError;
use crate::ServerResult;
use handlers::{
    prelude::{
        jwt::{BEARER_KEY, BEARER_VALUE},
        state::HandlerState,
    },
    get_asset,
};
use ppd_shared::tools::init_secrets;

fn to_origins(origins: &Option<Vec<String>>) -> AllowOrigin {
    match origins {
        Some(list) => {
            let headers: Vec<HeaderValue> = list
                .iter()
                .map(|s| match s.parse::<HeaderValue>() {
                    Ok(url) => Some(url),
                    Err(err) => {
                        tracing::error!("unable to pass cors origin {s}: {err}");
                        None
                    }
                })
                .flatten()
                .collect();

            headers.into()
        }
        None => AllowOrigin::any(),
    }
}

async fn create_app(config: &ServiceConfig) -> Result<IntoMakeService<Router<()>>, ServerError> {
    let state = HandlerState::new(config).await?;
    let origins = &config.base.allowed_origins;

    let cors = CorsLayer::new()
        .allow_origin(to_origins(origins))
        .allow_headers([
            ACCEPT,
            ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_static("x-ppd-client"),
        ])
        .allow_methods(Any);

    set_var(BEARER_KEY, BEARER_VALUE);

    println!("before loading router");
    let client_router = *get_client_router(config)?;

    println!(
        "after loading router, has routes {}",
        client_router.has_routes()
    );

    let router = Router::new()
        .route("/:asset_type/*asset_path", get(get_asset))
        .nest("/client", Router::new())
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    some_other_field = tracing::field::Empty,
                )
            }),
        )
        .layer(cors)
        .with_state(state)
        .into_make_service();

    println!("router created...");
    Ok(router)
}

type LoggerGuard = tracing_appender::non_blocking::WorkerGuard;
pub fn start_logger() -> ServerResult<LoggerGuard> {
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("ppd.log")?;
    let (writer, guard) = non_blocking(log_file);

    if let Err(err) = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppd_rest=debug,tower_http=debug".into()),
        )
        .with(fmt::layer().with_ansi(false).pretty().with_writer(writer))
        .try_init()
    {
        tracing::warn!("{err}")
    }

    Ok(guard)
}

pub async fn initialize_app(config: &ServiceConfig) -> ServerResult<IntoMakeService<Router<()>>> {
    // start ppdrive app
    init_secrets().await?;
    create_app(&config).await
}

fn get_client_router(config: &ServiceConfig) -> ServerResult<Box<Router<HandlerState>>> {
    let max_upload_size = config.base.max_upload_size;
    let router = if config.auth.modes.contains(&ServiceAuthMode::Client) {
        let svc_router = ServiceRouter {
            svc_type: ServiceType::Rest,
            auth_mode: ServiceAuthMode::Client,
        };

        println!("calling router get...");
        svc_router.get(max_upload_size)?
    } else {
        Box::new(Router::new())
    };

    Ok(router)
}

#[cfg(test)]
mod tests {
    use ppd_shared::opts::{ServiceAuthMode, ServiceConfig};

    use crate::{app::create_app, ServerResult};

    #[tokio::test]
    async fn test_create_app() -> ServerResult<()> {
        let mut config = ServiceConfig::default();
        config.base.db_url = "sqlite://db.sqlite".to_string();
        config.base.port = 5000;
        config.auth.modes.push(ServiceAuthMode::Client);

        let ca = create_app(&config).await;
        assert!(ca.is_ok());

        Ok(())
    }
}

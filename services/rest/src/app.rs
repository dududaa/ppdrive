use std::env::set_var;
use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue};
use axum::{extract::MatchedPath, http::Request, routing::get, Router};
use handlers::plugin::router::ServiceRouter;
use ppd_shared::opts::{ServiceAuthMode, ServiceConfig, ServiceType};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, Any};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;
use tracing_appender::non_blocking;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::ServerResult;
use handlers::{
    jwt::{BEARER_KEY, BEARER_VALUE},
    prelude::state::HandlerState,
    rest::get_asset,
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

async fn serve_app(config: &ServiceConfig, token: *mut CancellationToken) -> ServerResult<()> {
    tracing::debug!("preparing to serve app...");
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
    let client_router_ptr = get_client_router(config);
    let client_router = unsafe {
        if client_router_ptr.is_null() {
            &Router::new()
        } else {
            &*client_router_ptr
        }
    };

    let svc = Router::new()
        .route("/:asset_type/*asset_path", get(get_asset))
        .nest("/client", client_router.clone())
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

    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", &config.base.port)).await {
        Ok(listener) => {
            if let Ok(addr) = listener.local_addr() {
                tracing::info!("new service listening on {addr}");
            }

            if !token.is_null() {
                let token = unsafe { Box::from_raw(token) };
                tokio::select! {
                    _ = token.cancelled() => {},
                    _ = axum::serve(listener, svc) => {}
                }
            }

            // axum::serve(listener, svc)
            //     .await
            //     .map_err(|err| ServerError::InitError(err.to_string()))?;
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
            panic!("{err}")
        }
    }

    // drop client router
    unsafe {
        let _ = Box::from_raw(client_router_ptr);
    };

    Ok(())
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

pub async fn initialize_app(config: &ServiceConfig, token: *mut CancellationToken) -> ServerResult<()> {
    // start ppdrive app
    init_secrets().await?;
    serve_app(&config, token).await
}

fn get_client_router(config: &ServiceConfig) -> *mut Router<HandlerState> {
    let max_upload_size = config.base.max_upload_size;
    let mut router = std::ptr::null_mut();

    if config.auth.modes.contains(&ServiceAuthMode::Client) {
        let svc_router = ServiceRouter {
            svc_type: ServiceType::Rest,
            auth_mode: ServiceAuthMode::Client,
        };

        if let Ok(r) = svc_router.get(max_upload_size) {
            router = r;
        }
    }

    router
}

#[cfg(test)]
mod tests {
    use handlers::plugin::router::ServiceRouter;
    use ppd_shared::{
        opts::{ServiceAuthMode, ServiceConfig},
        plugin::{Plugin, PluginTransport},
    };
    use tokio_util::sync::CancellationToken;

    use crate::{app::serve_app, ServerResult};

    #[tokio::test]
    async fn test_create_app() -> ServerResult<()> {
        let svc = ServiceRouter::default();
        svc.preload()?;

        let mut config = ServiceConfig::default();
        config.base.db_url = "sqlite://db.sqlite".to_string();
        config.base.port = 5000;
        config.auth.modes.push(ServiceAuthMode::Client);

        let token = Box::new(CancellationToken::new());
        let token = Box::into_raw(token);
        let ca = serve_app(&config, token).await;
        if let Err(err) = &ca {
            println!("err: {err}")
        }

        assert!(ca.is_ok());

        Ok(())
    }
}

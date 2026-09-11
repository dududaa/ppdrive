//! HTTP application construction.
//!
//! Assembles the Axum [`Router`] with CORS, tracing, upload routes,
//! static file serving, and shared [`AppState`].

use crate::routers::{MetricsLayer, auth_routes, bucket_routes, download_serve_routes, download_sign_routes, upload_routes};
use crate::state::AppState;
use axum::Router;
use axum::extract::MatchedPath;
use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue, Request, StatusCode};
use axum::routing::{IntoMakeService, get};
use std::str::FromStr;
use std::time::Duration;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::info_span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};
use shared::db::bucket;
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::OnceLock;

/// Convert whitelisted url to axum AllowOrigin. When no url is provided, all origins will be allowed.
fn whitelist_to_origins(origins: &Option<Vec<String>>) -> AllowOrigin {
    match origins {
        Some(list) => {
            let headers: Vec<HeaderValue> = list
                .iter()
                .filter_map(|s| match s.parse::<HeaderValue>() {
                    Ok(url) => Some(url),
                    Err(err) => {
                        tracing::error!("unable to pass cors origin {s}: {err}");
                        None
                    }
                })
                .collect();

            headers.into()
        }
        None => AllowOrigin::any(),
    }
}

/// Build the complete Axum application: router, CORS, tracing, static dirs, state.
///
/// Returns the [`IntoMakeService`] and the configured port.
pub async fn create_app() -> anyhow::Result<(IntoMakeService<Router>, u16)> {
    start_logger()?;
    let state = AppState::new().await?;
    let origins = state.config().allowed_origins.clone();

    let client_header_key = state.config().client_header_key.clone();
    let port = state.config().port.unwrap_or(8000);

    if origins.is_none() {
        tracing::warn!("CORS: no allowed_origins configured, all origins are permitted");
    }

    let cors = CorsLayer::new()
        .allow_origin(whitelist_to_origins(&origins))
        .allow_headers([
            ACCEPT,
            ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_str(&client_header_key)?,
        ])
        .allow_methods(Any);

    // Per-IP rate limiter: 100 requests/sec with burst of 200
    let mut builder = GovernorConfigBuilder::default();
    builder.per_second(100).burst_size(200);
    let mut builder = builder.key_extractor(SmartIpKeyExtractor);
    let governor_conf = builder
        .finish()
        .ok_or_else(|| anyhow::anyhow!("failed to build rate limiter config"))?;

    let mut app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/metrics", get(metrics_handler))
        .nest("/auth", auth_routes())
        .nest("/buckets", bucket_routes())
        .nest("/upload", upload_routes())
        .nest("/download", download_sign_routes())
        .nest("/download", download_serve_routes())
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
        );

    let paths = bucket::get_public_paths(state.db()).await.unwrap_or_default();
    
    for path in &paths {
        app = app.nest_service(path, ServeDir::new(path));
    }
    
    for folder in state.config().static_folders.clone() {
        let path = folder.path.unwrap_or(format!("/{}", folder.name));
        app = app.nest_service(&path, ServeDir::new(folder.name));
    }

    let app = app
        .layer(cors)
        .layer(MetricsLayer)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(GovernorLayer::new(governor_conf))
        .with_state(state)
        .into_make_service();

    Ok((app, port))
}

/// Initialize the tracing subscriber with an env-filter (defaults to `info`).
fn start_logger() -> anyhow::Result<()> {
    if let Err(err) = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .try_init() {
        tracing::error!("logger error: {err}");
    }

    Ok(())
}

/// Prometheus metrics endpoint handler.
async fn metrics_handler() -> Result<String, StatusCode> {
    let handle = PROMETHEUS_HANDLE
        .get()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(handle.render())
}
static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the Prometheus recorder and store the handle for the `/metrics` endpoint.
pub fn install_metrics() -> anyhow::Result<()> {
    use metrics_exporter_prometheus::PrometheusBuilder;

    let builder = PrometheusBuilder::new();
    let handle = builder.install_recorder()?;
    PROMETHEUS_HANDLE
        .set(handle)
        .map_err(|_| anyhow::anyhow!("metrics already initialized"))?;
    Ok(())
}

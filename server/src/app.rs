use crate::routers::upload_routes;
use crate::state::AppState;
use axum::Router;
use axum::extract::MatchedPath;
use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue, Request};
use axum::routing::IntoMakeService;
use std::str::FromStr;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info_span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

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

pub async fn create_app() -> anyhow::Result<(IntoMakeService<Router>, i16)> {
    start_logger()?;
    let state = AppState::new().await?;
    let origins = state.config().allowed_origins.clone();

    let client_header_key = state.config().client_header_key.clone();
    let port = state.config().port.unwrap_or(8000);

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

    let app = Router::new()
        .nest("/upload", upload_routes())
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

    Ok((app, port))
}

fn start_logger() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace")))
        .with(fmt::layer())
        .try_init()?;

    Ok(())
}

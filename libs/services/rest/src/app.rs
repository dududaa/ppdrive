use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue};
use axum::{extract::MatchedPath, http::Request, routing::get, Router};
use ppdrive::plugin::router::Routers;
use ppd_shared::opts::internal::ServiceConfig;
use std::env::set_var;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, Any};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;

use crate::ServerResult;
use ppdrive::{
    jwt::{BEARER_KEY, BEARER_VALUE},
    prelude::state::HandlerState,
    rest::get_asset,
};

fn to_origins(origins: &Option<Vec<String>>) -> AllowOrigin {
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

pub async fn serve_app(
    config: Arc<ServiceConfig>,
    state: HandlerState,
    token: CancellationToken,
) -> ServerResult<()> {
    let origins = &config.base.allowed_origins;
    let cors = CorsLayer::new()
        .allow_origin(to_origins(origins))
        .allow_headers([
            ACCEPT,
            ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_static("ppd-client-token"),
        ])
        .allow_methods(Any);

    set_var(BEARER_KEY, BEARER_VALUE);
    let routers = Routers::from(config.clone()).load()?;

    let svc = Router::new()
        .route("/:asset_type/*asset_path", get(get_asset))
        .nest("/client", routers.client())
        .nest("/direct", routers.direct())
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

            tokio::select! {
                _ = token.cancelled() => {},
                _ = axum::serve(listener, svc) => {}
            }
        }
        Err(err) => {
            tracing::error!("Error starting listener: {err}");
        }
    }

    Ok(())
}

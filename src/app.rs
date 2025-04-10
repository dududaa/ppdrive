use axum::http::HeaderName;
use axum::{extract::MatchedPath, http::Request, routing::IntoMakeService, Router};
use reqwest::header::{
    HeaderValue, ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION,
    CONTENT_TYPE,
};
use tower_http::cors::Any;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;

use crate::routes::asset::asset_routes;
use crate::{errors::AppError, routes::admin::admin_routes, state::AppState, utils::get_env};

pub async fn create_app() -> Result<IntoMakeService<Router<()>>, AppError> {
    let state = AppState::new().await?;

    let wl = get_env("PPDRIVE_ALLOW_URL")?
        .parse::<HeaderValue>()
        .map_err(|err| AppError::InitError(err.to_string()))?;

    let cors = CorsLayer::new()
        .allow_origin(wl)
        .allow_headers([
            ACCEPT,
            ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static("x-client-id")
        ])
        .allow_methods(Any);

    let router = Router::new()
        .nest("/admin", admin_routes())
        .nest("/assets", asset_routes())
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                // Log the matched route's path (with placeholders not filled in).
                // Use request.uri() or OriginalUri if you want the real path.
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

    Ok(router)
}

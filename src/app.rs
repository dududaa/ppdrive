use std::env::set_var;

use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::HeaderName;
use axum::{
    extract::MatchedPath,
    http::Request,
    routing::{get, IntoMakeService},
    Router,
};
use tower_http::cors::Any;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;

use crate::config::AppConfig;
use crate::routes::client::client_routes;
use crate::routes::get_asset;
use crate::routes::protected::protected_routes;
use crate::utils::tools::secrets::{BEARER_KEY, BEARER_VALUE};
use crate::{errors::AppError, state::AppState};

pub async fn create_app(config: &AppConfig) -> Result<IntoMakeService<Router<()>>, AppError> {
    let state = AppState::new(config).await?;
    let origins = config.base().allowed_origins();

    let cors = CorsLayer::new()
        .allow_origin(origins)
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

    let router = Router::new()
        .route("/:asset_type/*asset_path", get(get_asset))
        .nest("/client", client_routes())
        .nest("/", protected_routes(config)?)
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

use axum::http::header::{
    HeaderValue, ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION,
    CONTENT_TYPE,
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

use crate::routes::client::client_routes;
use crate::routes::get_asset;
use crate::routes::manager::manager_routes;
use crate::{errors::AppError, state::AppState, utils::get_env};

pub async fn create_app() -> Result<IntoMakeService<Router<()>>, AppError> {
    let state = AppState::new().await?;

    let whitelist = get_env("PPDRIVE_ALLOWED_ORIGINS")?;
    let origins: Vec<HeaderValue> = whitelist.split(",").flat_map(|o| {
        match o.parse::<HeaderValue>() {
            Ok(h) => Some(h),
            Err(err) => {
                tracing::warn!("unable to parse origin {o}. Origin will not be whitelisted. \nmore info: {err}");
                None
            }
        }
    }).collect();

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

    let router = Router::new()
        .route("/:asset_type/*asset_path", get(get_asset))
        .nest("/client", client_routes())
        .nest("/", manager_routes())
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

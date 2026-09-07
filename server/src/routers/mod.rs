mod middlewares;
mod resp;
mod upload;

use self::upload::*;
use crate::state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::post;
use tower::limit::ConcurrencyLimitLayer;

const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024; // 2MB max upload
const MAX_CONCURRENT_REQUESTS: usize = 50; // max concurrent uploads

pub(crate) fn upload_routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/play/{payload}", post(play_session))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
}

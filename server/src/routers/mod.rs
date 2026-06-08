mod middlewares;
pub mod payloads;
mod resp;
mod session;
mod upload;

use self::upload::*;
use crate::state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{patch, post};

const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024; // 2MB max upload

pub(crate) fn upload_routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/play", post(play_session))
        .route("/session/next", patch(play_next_session))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT)) 
}

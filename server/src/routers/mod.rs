mod upload;
mod middlewares;
pub mod payloads;
mod resp;
mod session;

use crate::state::AppState;
use axum::routing::post;
use axum::Router;
use self::upload::*;

pub fn upload_routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/play", post(play_session))
}


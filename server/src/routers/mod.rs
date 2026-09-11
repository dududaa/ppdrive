//! HTTP route definitions.
//!
//! Composes upload and download session endpoints with body-size and concurrency limits.

mod auth;
mod bucket;
mod download;
mod middlewares;
mod metrics;
mod permissions;
mod resp;
mod upload;

use self::auth::*;
use self::bucket::*;
use self::download::*;
use self::permissions::*;
use self::upload::*;
pub(crate) use self::metrics::MetricsLayer;
use crate::state::AppState;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
use tower::limit::ConcurrencyLimitLayer;

const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024; // 2MB max upload
const MAX_CONCURRENT_REQUESTS: usize = 50; // max concurrent uploads

/// Build the `/auth` router (public, no auth required).
pub(crate) fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
}

/// Build the `/upload` router with session endpoints, body limit, and concurrency cap.
pub(crate) fn upload_routes() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/play/{payload}", post(play_session))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
}

/// Build the `/download/sign` router (requires client auth header).
pub(crate) fn download_sign_routes() -> Router<AppState> {
    Router::new()
        .route("/sign", post(sign_download))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
}

/// Build the `/download/{token}` router (public, no auth header needed).
pub(crate) fn download_serve_routes() -> Router<AppState> {
    Router::new()
        .route("/{token}", get(serve_download))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
}

/// Build the `/buckets` router (requires client auth header).
pub(crate) fn bucket_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_bucket))
        .route("/{bucket_pid}/permissions", post(grant_permission))
        .route("/{bucket_pid}/permissions", delete(revoke_permission))
        .route("/{bucket_pid}/permissions", get(list_permissions))
        .layer(ConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
}

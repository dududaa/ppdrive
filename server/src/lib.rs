//! PPDRIVE HTTP server crate.
//!
//! Provides the Axum-based HTTP server, including upload routing, middleware for
//! client authentication and session verification, and application state management.

pub mod app;
pub mod routers;
pub mod state;

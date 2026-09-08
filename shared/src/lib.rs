//! Core shared library for PPDRIVE.
//!
//! This crate provides the common types, database access, cryptographic utilities,
//! and configuration parsing used by both the HTTP server and the CLI.

#[cfg(feature = "server")]
pub mod broker;
pub mod db;
#[cfg(feature = "server")]
pub mod server;
pub mod user;

mod tools;
pub use tools::*;
pub use db::utils::{AssetOwnerName, check_ownership};
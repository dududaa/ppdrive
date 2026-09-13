//! Core shared library for PPDRIVE.
//!
//! This crate provides the common types, database access, cryptographic utilities,
//! and configuration parsing used by both the HTTP server and the CLI.

#[cfg(feature = "server")]
pub mod broker;
pub mod db;
#[cfg(feature = "server")]
pub mod server;

mod utils;


mod tools;
pub use tools::*;
pub use utils::{AssetOwnerName, asset_owner_id, check_ownership, seconds_from_now};
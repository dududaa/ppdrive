//! functionalities shared by server handlers

use crate::errors::HandlerError;

pub mod errors;

// #[cfg(feature = "prelude")]
// pub use crate::prelude::{extractors::ClientUser, state::HandlerState};

#[cfg(feature = "prelude")]
pub mod prelude;

#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "tools")]
pub mod tools;

#[cfg(feature = "rest")]
pub mod rest;

pub type HandlerResult<T> = Result<T, HandlerError>;

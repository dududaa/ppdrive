//! functionalities shared by server handlers

use crate::errors::HandlerError;

pub mod errors;

pub mod prelude;

#[cfg(feature = "jwt")]
pub mod jwt;

#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "tools")]
pub mod tools;

#[cfg(feature = "rest")]
pub mod rest;

pub type HandlerResult<T> = Result<T, HandlerError>;

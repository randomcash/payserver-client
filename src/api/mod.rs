//! API client for a payserver.

pub(crate) mod client;
mod types;

pub use self::types::*;
pub use client::{ApiClient, ApiError, SafeModeStatus};

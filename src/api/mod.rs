//! API client for a payserver.

// pub(crate), not private: `wallet_tab`'s tests reach `client::{RequestSpec,
// TestTransport}` directly to drive `fetch_stores_sharing_wallet` through the
// same transport seam `stores`'s own tests use, rather than re-deriving it.
pub(crate) mod client;
mod types;

pub use self::types::*;
pub use client::{ApiClient, ApiError};

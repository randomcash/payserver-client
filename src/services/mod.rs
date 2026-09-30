//! Client-side services.

pub mod eip6963;
pub mod websocket;

pub use eip6963::{DiscoveredWallet, discover_wallets, provider_request};
pub use websocket::{ConnectionState, StatusUpdate, WebSocketService};

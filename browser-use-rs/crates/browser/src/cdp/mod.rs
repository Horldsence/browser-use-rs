//! CDP (Chrome DevTools Protocol) Client Implementation
//!
//! Core principle: Single WebSocket connection, multiplexed sessions.
//! No locks in hot path - use message passing instead.

pub mod client;
pub mod protocol;
pub mod session;

pub use client::CDPClient;
pub use protocol::{CDPEvent, CDPRequest, CDPResponse};
pub use session::CDPSession;

//! Browser Session Management - Rust Implementation
//!
//! This crate provides a Rust implementation of browser-use's browser session management,
//! focusing on CDP (Chrome DevTools Protocol) communication and session lifecycle.
//!
//! # Architecture Philosophy (Linus-approved)
//!
//! 1. **Data structures first**: Clean ownership model, no unnecessary copies
//! 2. **Zero special cases**: Generic handling through traits, not if/else chains  
//! 3. **Backward compatible**: Can be used via FFI from Python without breaking existing code
//! 4. **Practical**: Solve real performance bottlenecks, not imaginary ones

pub mod cdp;
pub mod events;
pub mod session;
pub mod watchdog;
pub mod watchdogs;

pub use cdp::{CDPClient, CDPSession};
pub use events::EventBus;
pub use session::{BrowserSession, SessionConfig};
pub use watchdog::{Watchdog, WatchdogManager};
pub use watchdogs::CrashWatchdog;

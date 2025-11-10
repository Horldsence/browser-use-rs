//! Concrete Watchdog Implementations
//!
//! Each watchdog is a separate module for clarity.

pub mod crash;
pub mod downloads;
pub mod security;

// Re-export for convenience
pub use crash::CrashWatchdog;
pub use downloads::DownloadsWatchdog;
pub use security::{SecurityPolicy, SecurityWatchdog};

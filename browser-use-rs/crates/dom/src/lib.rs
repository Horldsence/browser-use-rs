//! Browser-Use DOM Processing Library
//!
//! High-performance DOM tree processing with zero-copy design.
//!
//! ## Philosophy (Linus Torvalds Style)
//!
//! - **Good taste**: Data structures first, algorithms follow naturally
//! - **No special cases**: Type system eliminates branches
//! - **Zero copy**: Borrowed data everywhere, owned only when necessary
//! - **Cache friendly**: Arena allocation, sequential access patterns
//!
//! ## Core Design
//!
//! ```text
//! CDP JSON → RawNode (borrowed) → DomArena (owned) → Iterator → Serialized
//!                                      ↓
//!                               NodeIndex (u32)
//! ```

pub mod arena;
pub mod error;
pub mod serializer;
pub mod service;
pub mod types;
pub mod utils;

pub use arena::DomArena;
pub use error::{DomError, Result};
pub use service::DomService;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_node_creation() {
        let _arena = DomArena::new();
        // More tests will be added as we implement
    }
}

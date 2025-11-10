//! Error types for DOM operations
//!
//! Simple, flat error hierarchy. No over-engineering.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DomError>;

#[derive(Debug, Error)]
pub enum DomError {
    #[error("Node not found: {0}")]
    NodeNotFound(u32),

    #[error("Invalid node type: expected {expected}, got {actual}")]
    InvalidNodeType { expected: String, actual: String },

    #[error("CDP protocol error: {0}")]
    CdpError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Parse error: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Timeout waiting for page: {0}")]
    Timeout(String),

    #[error("Maximum iframe depth exceeded: {current} > {max}")]
    MaxIframeDepthExceeded { current: usize, max: usize },

    #[error("Maximum iframe count exceeded: {current} > {max}")]
    MaxIframeCountExceeded { current: usize, max: usize },
}

//! CDP Protocol Types
//!
//! These are the fundamental types for CDP communication.
//! Keep them minimal - add domain-specific types only when needed.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request ID - monotonically increasing
pub type RequestId = u64;

/// Target ID from Chrome
pub type TargetId = String;

/// Session ID for attached targets
pub type SessionId = String;

/// CDP Request sent to browser
#[derive(Debug, Clone, Serialize)]
pub struct CDPRequest {
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
}

/// CDP Response from browser
#[derive(Debug, Clone, Deserialize)]
pub struct CDPResponse {
    pub id: RequestId,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<CDPError>,
}

/// CDP Error
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CDPError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// CDP Event from browser (no request ID)
#[derive(Debug, Clone, Deserialize)]
pub struct CDPEvent {
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(rename = "sessionId", default)]
    pub session_id: Option<SessionId>,
}

/// Unified CDP Message (request, response, or event)
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CDPMessage {
    Response(CDPResponse),
    Event(CDPEvent),
}

/// Target Info from Target.getTargetInfo
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetInfo {
    #[serde(rename = "targetId")]
    pub target_id: TargetId,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub attached: bool,
}

/// Result of Target.attachToTarget
#[derive(Debug, Clone, Deserialize)]
pub struct AttachToTargetResult {
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
}

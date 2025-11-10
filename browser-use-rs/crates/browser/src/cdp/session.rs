//! CDP Session - Represents a connection to a specific browser target
//!
//! Design: Lightweight wrapper around CDPClient with target-specific context.
//! All sessions share the same WebSocket - no per-session connection overhead.

use super::client::{CDPClient, Result};
use super::protocol::{AttachToTargetResult, SessionId, TargetId, TargetInfo};
use serde_json::{Value, json};
use std::sync::Arc;

/// CDP Session bound to a specific target
#[derive(Clone)]
pub struct CDPSession {
    /// Shared CDP client
    client: Arc<CDPClient>,

    /// Target this session is attached to
    pub target_id: TargetId,

    /// Session ID assigned by Chrome
    pub session_id: SessionId,

    /// Cached target info
    pub title: String,
    pub url: String,
}

impl CDPSession {
    /// Attach to a target and create session
    pub async fn attach(
        client: Arc<CDPClient>,
        target_id: TargetId,
        domains: Option<Vec<&str>>,
    ) -> Result<Self> {
        // Attach to target
        let result = client
            .send_request(
                "Target.attachToTarget",
                Some(json!({
                    "targetId": target_id,
                    "flatten": true,
                })),
                None,
            )
            .await?;

        let attach_result: AttachToTargetResult =
            serde_json::from_value(result).map_err(|e| super::client::CDPError::Json(e))?;

        let session_id = attach_result.session_id;

        // Enable domains (default set if not specified)
        let domains = domains.unwrap_or_else(|| {
            vec![
                "Page",
                "DOM",
                "DOMSnapshot",
                "Accessibility",
                "Runtime",
                "Inspector",
            ]
        });

        // Enable all domains in parallel
        let enable_futures: Vec<_> = domains
            .into_iter()
            .map(|domain| {
                let client = client.clone();
                let session_id = session_id.clone();
                async move {
                    client
                        .send_request(format!("{}.enable", domain), None, Some(session_id))
                        .await
                }
            })
            .collect();

        // Wait for all enables (ignore individual failures)
        let results = futures_util::future::join_all(enable_futures).await;
        let failures: Vec<_> = results.iter().filter(|r| r.is_err()).collect();
        if !failures.is_empty() {
            tracing::warn!(
                "Some domain enables failed: {}/{}",
                failures.len(),
                results.len()
            );
        }

        // Disable breakpoints if Debugger domain was enabled
        let _ = client
            .send_request(
                "Debugger.setBreakpointsActive",
                Some(json!({ "active": false })),
                Some(session_id.clone()),
            )
            .await; // Ignore error

        // Get target info
        let info_result = client
            .send_request(
                "Target.getTargetInfo",
                Some(json!({ "targetId": &target_id })),
                None,
            )
            .await?;

        let target_info: TargetInfo = serde_json::from_value(info_result["targetInfo"].clone())
            .map_err(|e| super::client::CDPError::Json(e))?;

        Ok(Self {
            client,
            target_id,
            session_id,
            title: target_info.title,
            url: target_info.url,
        })
    }

    /// Send command within this session's context
    pub async fn send(&self, method: impl Into<String>, params: Option<Value>) -> Result<Value> {
        self.client
            .send_request(method, params, Some(self.session_id.clone()))
            .await
    }

    /// Get current target info
    pub async fn get_target_info(&self) -> Result<TargetInfo> {
        let result = self
            .client
            .send_request(
                "Target.getTargetInfo",
                Some(json!({ "targetId": &self.target_id })),
                None,
            )
            .await?;

        serde_json::from_value(result["targetInfo"].clone())
            .map_err(|e| super::client::CDPError::Json(e))
    }

    /// Navigate to URL
    pub async fn navigate(&self, url: impl Into<String>) -> Result<Value> {
        self.send("Page.navigate", Some(json!({ "url": url.into() })))
            .await
    }

    /// Evaluate JavaScript
    pub async fn evaluate(&self, expression: impl Into<String>) -> Result<Value> {
        let result = self
            .send(
                "Runtime.evaluate",
                Some(json!({
                    "expression": expression.into(),
                    "returnByValue": true,
                })),
            )
            .await?;

        Ok(result)
    }
}

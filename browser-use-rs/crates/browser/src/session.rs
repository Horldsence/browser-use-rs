//! Browser Session Management
//!
//! This is the high-level API that Python code will interact with.
//! Manages browser lifecycle, tabs, and state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::cdp::protocol::TargetId;
use crate::cdp::{CDPClient, CDPSession};
use crate::events::{BrowserEvent, EventBus};
use crate::watchdog::WatchdogManager;
use crate::watchdogs::{CrashWatchdog, DownloadsWatchdog, SecurityWatchdog};
use std::path::PathBuf;

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub id: String,
    pub cdp_url: String,
    pub headless: bool,
    pub user_data_dir: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            cdp_url: "ws://localhost:9222".to_string(),
            headless: true,
            user_data_dir: None,
        }
    }
}

/// Browser Session - manages connection to Chrome and tabs
pub struct BrowserSession {
    pub config: SessionConfig,
    pub event_bus: EventBus,

    // CDP infrastructure
    cdp_client: Arc<RwLock<Option<Arc<CDPClient>>>>,
    sessions: Arc<RwLock<HashMap<TargetId, CDPSession>>>,

    // Current focus
    current_target: Arc<RwLock<Option<TargetId>>>,

    // Watchdog system - replaces Python's 11 separate fields
    watchdog_manager: Arc<RwLock<WatchdogManager>>,
}

impl BrowserSession {
    pub fn new(config: SessionConfig) -> Self {
        // Initialize watchdog manager with default watchdogs
        let mut watchdog_manager = WatchdogManager::new();

        // Core watchdogs enabled by default
        watchdog_manager.register(Box::new(CrashWatchdog::new()));

        // Downloads watchdog - uses /tmp/browser-downloads by default
        let downloads_dir = PathBuf::from("/tmp/browser-downloads");
        watchdog_manager.register(Box::new(DownloadsWatchdog::new(downloads_dir)));

        // Security watchdog - allow all by default (no restrictions)
        watchdog_manager.register(Box::new(SecurityWatchdog::new()));

        Self {
            config,
            event_bus: EventBus::new(),
            cdp_client: Arc::new(RwLock::new(None)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            current_target: Arc::new(RwLock::new(None)),
            watchdog_manager: Arc::new(RwLock::new(watchdog_manager)),
        }
    }

    /// Start the browser session
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Connect to CDP
        let client = CDPClient::connect(&self.config.cdp_url).await?;
        let client_arc = Arc::clone(&client);
        *self.cdp_client.write().await = Some(client);

        // Attach watchdogs with CDP client
        self.watchdog_manager
            .read()
            .await
            .attach_all(client_arc)
            .await?;

        // Publish event and dispatch to watchdogs
        let event = Arc::new(BrowserEvent::Started);
        self.event_bus.publish((*event).clone());
        self.watchdog_manager.read().await.dispatch(event).await;

        Ok(())
    }

    /// Stop the browser session
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Detach watchdogs
        self.watchdog_manager.read().await.detach_all().await?;

        // Close all sessions
        self.sessions.write().await.clear();

        // Close CDP client
        if let Some(client) = self.cdp_client.write().await.take() {
            client.close().await?;
        }

        // Publish event and dispatch to watchdogs
        let event = Arc::new(BrowserEvent::Stopped);
        self.event_bus.publish((*event).clone());
        self.watchdog_manager.read().await.dispatch(event).await;

        Ok(())
    }

    /// Create new tab
    pub async fn new_tab(
        &self,
        url: Option<String>,
    ) -> Result<TargetId, Box<dyn std::error::Error>> {
        let client = self
            .cdp_client
            .read()
            .await
            .as_ref()
            .ok_or("Not connected")?
            .clone();

        let url = url.unwrap_or_else(|| "about:blank".to_string());

        let result = client
            .send_request(
                "Target.createTarget",
                Some(serde_json::json!({ "url": url })),
                None,
            )
            .await?;

        let target_id: TargetId = result["targetId"]
            .as_str()
            .ok_or("Invalid targetId")?
            .to_string();

        // Attach to the new target
        let session = CDPSession::attach(client, target_id.clone(), None).await?;

        self.sessions
            .write()
            .await
            .insert(target_id.clone(), session);
        *self.current_target.write().await = Some(target_id.clone());

        // Publish event and dispatch to watchdogs
        let event = Arc::new(BrowserEvent::TabCreated {
            target_id: target_id.clone(),
        });
        self.event_bus.publish((*event).clone());
        self.watchdog_manager.read().await.dispatch(event).await;

        Ok(target_id)
    }

    /// Switch to tab
    pub async fn switch_tab(&self, target_id: TargetId) -> Result<(), Box<dyn std::error::Error>> {
        let sessions = self.sessions.read().await;
        if !sessions.contains_key(&target_id) {
            return Err("Target not found".into());
        }

        *self.current_target.write().await = Some(target_id.clone());

        // Publish event and dispatch to watchdogs
        let event = Arc::new(BrowserEvent::TabSwitched {
            target_id: target_id.clone(),
        });
        self.event_bus.publish((*event).clone());
        self.watchdog_manager.read().await.dispatch(event).await;

        Ok(())
    }

    /// Get current session
    pub async fn current_session(&self) -> Option<CDPSession> {
        let target_id = self.current_target.read().await.clone()?;
        self.sessions.read().await.get(&target_id).cloned()
    }

    /// Navigate current tab
    pub async fn navigate(&self, url: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
        let url = url.into();
        let session = self.current_session().await.ok_or("No active session")?;

        // Publish navigation started event
        let event_start = Arc::new(BrowserEvent::NavigationStarted { url: url.clone() });
        self.event_bus.publish((*event_start).clone());
        self.watchdog_manager
            .read()
            .await
            .dispatch(event_start)
            .await;

        session.navigate(&url).await?;

        // Publish navigation complete event
        let event_complete = Arc::new(BrowserEvent::NavigationComplete { url: url.clone() });
        self.event_bus.publish((*event_complete).clone());
        self.watchdog_manager
            .read()
            .await
            .dispatch(event_complete)
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Needs running Chrome
    async fn test_session_lifecycle() {
        let config = SessionConfig::default();
        let session = BrowserSession::new(config);

        session.start().await.unwrap();

        let target_id = session
            .new_tab(Some("https://example.com".to_string()))
            .await
            .unwrap();

        println!("Created tab: {}", target_id);

        session.stop().await.unwrap();
    }
}

//! Watchdog System - Unified Browser Monitoring
//!
//! Eliminates Python's 11-field mess with a clean trait-based design.
//!
//! Philosophy:
//! - Composition > Inheritance
//! - No reflection magic
//! - Zero-cost event dispatch

use async_trait::async_trait;
use std::sync::Arc;

use crate::cdp::CDPClient;
use crate::events::BrowserEvent;

/// Watchdog trait - monitors browser state and reacts to events
///
/// Python has 11 separate watchdog fields on BrowserSession.
/// Rust has a Vec<Box<dyn Watchdog>>.
///
/// **Linus says**: "Good data structures eliminate special cases."
#[async_trait]
pub trait Watchdog: Send + Sync {
    /// Human-readable name for logging
    fn name(&self) -> &str;

    /// Handle browser event
    ///
    /// This is called for EVERY event. Watchdog decides what to care about.
    /// Don't worry about performance - event dispatch is cheap.
    async fn on_event(&self, event: &BrowserEvent);

    /// Optional: Called when watchdog is first attached
    ///
    /// The watchdog receives a CDPClient reference to subscribe to CDP events
    /// and send CDP commands.
    ///
    /// # Example
    /// ```ignore
    /// async fn on_attach(&self, cdp_client: Arc<CDPClient>) -> Result<...> {
    ///     let state = self.state.clone();
    ///     cdp_client.subscribe("Inspector.targetCrashed", Arc::new(move |event| {
    ///         let state = state.clone();
    ///         tokio::spawn(async move {
    ///             // Handle crash event asynchronously
    ///         });
    ///     }));
    ///     Ok(())
    /// }
    /// ```
    async fn on_attach(
        &self,
        cdp_client: Arc<CDPClient>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = cdp_client; // Suppress unused warning for default impl
        Ok(())
    }

    /// Optional: Called when watchdog is detached (session stop)
    async fn on_detach(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Watchdog manager - dispatches events to all watchdogs
pub struct WatchdogManager {
    watchdogs: Vec<Box<dyn Watchdog>>,
}

impl WatchdogManager {
    pub fn new() -> Self {
        Self {
            watchdogs: Vec::new(),
        }
    }

    /// Add a watchdog
    pub fn register(&mut self, watchdog: Box<dyn Watchdog>) {
        tracing::debug!("Registered watchdog: {}", watchdog.name());
        self.watchdogs.push(watchdog);
    }

    /// Attach all watchdogs
    pub async fn attach_all(
        &self,
        cdp_client: Arc<CDPClient>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for watchdog in &self.watchdogs {
            watchdog.on_attach(cdp_client.clone()).await?;
        }
        Ok(())
    }

    /// Detach all watchdogs
    pub async fn detach_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        for watchdog in &self.watchdogs {
            watchdog.on_detach().await?;
        }
        Ok(())
    }

    /// Dispatch event to all watchdogs in parallel
    ///
    /// Python does this sequentially with asyncio.gather.
    /// Rust does it with join_all for true parallelism.
    pub async fn dispatch(&self, event: Arc<BrowserEvent>) {
        use futures_util::future::join_all;

        let tasks: Vec<_> = self
            .watchdogs
            .iter()
            .map(|w| {
                let event = event.clone();
                async move {
                    w.on_event(&event).await;
                }
            })
            .collect();

        join_all(tasks).await;
    }
}

impl Default for WatchdogManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestWatchdog {
        name: String,
        event_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Watchdog for TestWatchdog {
        fn name(&self) -> &str {
            &self.name
        }

        async fn on_event(&self, _event: &BrowserEvent) {
            self.event_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_watchdog_dispatch() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::Arc;

        let counter = Arc::new(AtomicUsize::new(0));
        let mut manager = WatchdogManager::new();

        manager.register(Box::new(TestWatchdog {
            name: "test1".to_string(),
            event_count: counter.clone(),
        }));
        manager.register(Box::new(TestWatchdog {
            name: "test2".to_string(),
            event_count: counter.clone(),
        }));

        let event = Arc::new(BrowserEvent::Started);
        manager.dispatch(event).await;

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}

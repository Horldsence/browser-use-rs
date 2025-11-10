//! Crash Watchdog - Monitors browser crashes and network timeouts
//!
//! Python version has 336 lines of spaghetti code.
//! Let's do better.

use async_trait::async_trait;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::cdp::{CDPClient, CDPSession};
use crate::events::BrowserEvent;
use crate::watchdog::Watchdog;

/// Tracks a single network request
#[derive(Clone, Debug)]
struct RequestTracker {
    request_id: String,
    start_time: Instant,
    url: String,
    method: String,
}

/// Crash Watchdog - detects page crashes and hung requests
pub struct CrashWatchdog {
    /// Timeout for network requests (seconds)
    network_timeout: Duration,

    /// Check interval for monitoring (seconds)
    check_interval: Duration,

    /// Active network requests - using Arc<RwLock<Vec>> for simplicity
    active_requests: Arc<RwLock<Vec<RequestTracker>>>,

    /// CDP sessions being monitored
    sessions: Arc<RwLock<Vec<Arc<CDPSession>>>>,

    /// Monitoring task handle
    monitor_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl CrashWatchdog {
    pub fn new() -> Self {
        Self {
            network_timeout: Duration::from_secs(10),
            check_interval: Duration::from_secs(5),
            active_requests: Arc::new(RwLock::new(Vec::new())),
            sessions: Arc::new(RwLock::new(Vec::new())),
            monitor_task: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_timeout(network_timeout: Duration, check_interval: Duration) -> Self {
        Self {
            network_timeout,
            check_interval,
            active_requests: Arc::new(RwLock::new(Vec::new())),
            sessions: Arc::new(RwLock::new(Vec::new())),
            monitor_task: Arc::new(RwLock::new(None)),
        }
    }

    /// Start monitoring loop
    async fn start_monitoring(&self) {
        let active_requests = self.active_requests.clone();
        let network_timeout = self.network_timeout;
        let check_interval = self.check_interval;

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            loop {
                interval.tick().await;

                // Check for timed out requests
                let now = Instant::now();
                let mut requests = active_requests.write().await;

                // Find and remove timed out requests
                let mut i = 0;
                while i < requests.len() {
                    let elapsed = now.duration_since(requests[i].start_time);
                    if elapsed > network_timeout {
                        let tracker = requests.remove(i);
                        tracing::warn!(
                            "[CrashWatchdog] Request timeout after {:?}: {}",
                            elapsed,
                            tracker.url
                        );
                    } else {
                        i += 1;
                    }
                }
            }
        });

        *self.monitor_task.write().await = Some(task);
    }

    /// Stop monitoring loop
    async fn stop_monitoring(&self) {
        if let Some(task) = self.monitor_task.write().await.take() {
            task.abort();
        }
        self.active_requests.write().await.clear();
    }

    /// Track new network request
    async fn track_request(&self, request_id: String, url: String, method: String) {
        let tracker = RequestTracker {
            request_id: request_id.clone(),
            start_time: Instant::now(),
            url,
            method,
        };
        self.active_requests.write().await.push(tracker);
    }

    /// Remove request from tracking
    async fn untrack_request(&self, request_id: &str) {
        let mut requests = self.active_requests.write().await;
        if let Some(pos) = requests.iter().position(|r| r.request_id == request_id) {
            let tracker = requests.remove(pos);
            let elapsed = Instant::now().duration_since(tracker.start_time);
            tracing::debug!(
                "[CrashWatchdog] Request completed in {:?}: {}",
                elapsed,
                tracker.url
            );
        }
    }

    /// Get active request count (for testing)
    pub async fn active_request_count(&self) -> usize {
        self.active_requests.read().await.len()
    }
}

impl Default for CrashWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Watchdog for CrashWatchdog {
    fn name(&self) -> &str {
        "CrashWatchdog"
    }

    async fn on_event(&self, event: &BrowserEvent) {
        match event {
            BrowserEvent::Started => {
                tracing::debug!("[CrashWatchdog] Browser started, beginning monitoring");
                self.start_monitoring().await;
            }

            BrowserEvent::Stopped => {
                tracing::debug!("[CrashWatchdog] Browser stopped, ending monitoring");
                self.stop_monitoring().await;
            }

            BrowserEvent::TabCreated { target_id } => {
                tracing::debug!("[CrashWatchdog] Tab created: {}", target_id);
                // TODO: Attach to new target and register CDP event handlers
                // This requires access to CDPClient, which we'll add in integration phase
            }

            BrowserEvent::TabClosed { target_id } => {
                tracing::debug!("[CrashWatchdog] Tab closed: {}", target_id);
                // Clean up tracking for this target
            }

            _ => {
                // Ignore other events
            }
        }
    }

    async fn on_attach(
        &self,
        cdp_client: Arc<CDPClient>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("[CrashWatchdog] Attaching to CDP");

        // Subscribe to crash events
        let _active_requests = self.active_requests.clone();
        cdp_client.subscribe(
            "Inspector.targetCrashed",
            Arc::new(move |event| {
                tracing::warn!("[CrashWatchdog] 💥 Target crashed: {:?}", event.params);
                // TODO: Implement crash recovery (reload page, notify user)
            }),
        );

        // Subscribe to network events for timeout tracking
        let requests_clone = self.active_requests.clone();
        cdp_client.subscribe(
            "Network.requestWillBeSent",
            Arc::new(move |event| {
                let requests = requests_clone.clone();
                tokio::spawn(async move {
                    if let Some(params) = event.params.as_ref() {
                        let request_id = params["requestId"].as_str().unwrap_or("").to_string();
                        let url = params["request"]["url"].as_str().unwrap_or("").to_string();
                        let method = params["request"]["method"]
                            .as_str()
                            .unwrap_or("GET")
                            .to_string();

                        let tracker = RequestTracker {
                            request_id: request_id.clone(),
                            start_time: Instant::now(),
                            url: url.clone(),
                            method,
                        };

                        requests.write().await.push(tracker);
                        tracing::debug!("[CrashWatchdog] Tracking request {}: {}", request_id, url);
                    }
                });
            }),
        );

        // Subscribe to response events
        let requests_clone = self.active_requests.clone();
        cdp_client.subscribe(
            "Network.responseReceived",
            Arc::new(move |event| {
                let requests = requests_clone.clone();
                tokio::spawn(async move {
                    if let Some(params) = event.params.as_ref() {
                        let request_id = params["requestId"].as_str().unwrap_or("");
                        let mut requests_guard = requests.write().await;
                        if let Some(pos) = requests_guard
                            .iter()
                            .position(|r| r.request_id == request_id)
                        {
                            let tracker = requests_guard.remove(pos);
                            let elapsed = Instant::now().duration_since(tracker.start_time);
                            tracing::debug!(
                                "[CrashWatchdog] Request completed in {:?}: {}",
                                elapsed,
                                tracker.url
                            );
                        }
                    }
                });
            }),
        );

        // Subscribe to failed request events
        let requests_clone = self.active_requests.clone();
        cdp_client.subscribe(
            "Network.loadingFailed",
            Arc::new(move |event| {
                let requests = requests_clone.clone();
                tokio::spawn(async move {
                    if let Some(params) = event.params.as_ref() {
                        let request_id = params["requestId"].as_str().unwrap_or("");
                        let mut requests_guard = requests.write().await;
                        if let Some(pos) = requests_guard
                            .iter()
                            .position(|r| r.request_id == request_id)
                        {
                            let tracker = requests_guard.remove(pos);
                            let elapsed = Instant::now().duration_since(tracker.start_time);
                            tracing::warn!(
                                "[CrashWatchdog] Request failed after {:?}: {}",
                                elapsed,
                                tracker.url
                            );
                        }
                    }
                });
            }),
        );

        tracing::info!("[CrashWatchdog] Successfully attached to CDP events");
        Ok(())
    }

    async fn on_detach(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop_monitoring().await;
        tracing::info!("[CrashWatchdog] Detached");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_crash_watchdog_lifecycle() {
        let watchdog = CrashWatchdog::new();

        // Create mock CDP client (skip attach for this test)
        // In real usage, attach would be called with actual CDPClient

        // Simulate browser start
        let event = BrowserEvent::Started;
        watchdog.on_event(&event).await;

        // Simulate network request
        watchdog
            .track_request(
                "req1".to_string(),
                "https://example.com".to_string(),
                "GET".to_string(),
            )
            .await;

        assert_eq!(watchdog.active_request_count().await, 1);

        // Complete request
        watchdog.untrack_request("req1").await;
        assert_eq!(watchdog.active_request_count().await, 0);

        // Test detach
        watchdog.on_detach().await.unwrap();
    }

    #[tokio::test]
    async fn test_request_timeout() {
        let watchdog =
            CrashWatchdog::with_timeout(Duration::from_millis(100), Duration::from_millis(50));

        // Start monitoring directly (simulating Started event)
        watchdog.on_event(&BrowserEvent::Started).await;

        // Add a request
        watchdog
            .track_request(
                "slow_req".to_string(),
                "https://slow.example.com".to_string(),
                "GET".to_string(),
            )
            .await;

        assert_eq!(watchdog.active_request_count().await, 1);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Request should be removed by timeout
        assert_eq!(watchdog.active_request_count().await, 0);

        watchdog.stop_monitoring().await;
    }
}
